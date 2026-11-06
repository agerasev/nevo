//! The complete scalar history stays in memory; only newly appended records are read.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use anyhow::Result;
use ciborium::Value;

use crate::metrics::log::{Record, RunReader, Sample};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Window {
    pub start_tick: u64,
    pub end_tick: u64,
    pub samples: u64,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Point {
    pub tick: u64,
    pub value: f64,
    pub window: Option<Window>,
}

#[derive(Clone, Debug)]
pub(super) struct Segment {
    pub session: u64,
    pub name: String,
    pub points: Vec<Point>,
}

struct Snapshot {
    session: u64,
    tick: u64,
    values: BTreeMap<String, f64>,
    window: Option<Window>,
}

pub(super) struct History {
    reader: RunReader,
    snapshots: Vec<Snapshot>,
    names: BTreeSet<String>,
    sessions: BTreeMap<u64, String>,
    extent: Option<[u64; 2]>,
}

impl History {
    pub fn new(path: PathBuf) -> Self {
        Self {
            reader: RunReader::new(path),
            snapshots: Vec::new(),
            names: BTreeSet::new(),
            sessions: BTreeMap::new(),
            extent: None,
        }
    }

    /// One bounded batch. Loaded values are never evicted or reread for plotting.
    pub fn poll(&mut self) -> Result<bool> {
        let records = self.reader.poll()?;
        let changed = !records.is_empty();
        for record in records {
            match record {
                Record::Session(session) => {
                    self.sessions.insert(
                        session.id,
                        format!("Session {} · from tick {}", session.id, session.start_tick),
                    );
                }
                Record::Sample(sample) => self.push(sample),
            }
        }
        Ok(changed)
    }

    fn push(&mut self, sample: Sample) {
        let mut values = BTreeMap::new();
        collect_scalars(&sample.values, "", &mut values);
        self.names.extend(values.keys().cloned());
        self.extent = Some(match self.extent {
            None => [sample.tick, sample.tick],
            Some([min, max]) => [min.min(sample.tick), max.max(sample.tick)],
        });
        self.snapshots.push(Snapshot {
            session: sample.session,
            tick: sample.tick,
            window: window(&sample.values),
            values,
        });
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    pub fn total_windows(&self) -> u64 {
        self.snapshots.len() as u64
    }

    pub fn last_tick(&self) -> Option<u64> {
        self.snapshots.last().map(|snapshot| snapshot.tick)
    }

    pub fn extent(&self) -> Option<[u64; 2]> {
        self.extent
    }

    pub fn has_pending_record(&self) -> bool {
        self.reader.has_pending_record()
    }

    /// Downsample only the drawing. Original samples remain available at every
    /// zoom level, with no file access. Gaps and sessions always break curves.
    pub fn query(&self, name: &str, range: Option<[f64; 2]>, pixels: usize) -> Vec<Segment> {
        let Some(extent) = self.extent else {
            return Vec::new();
        };
        let mut range = range.unwrap_or([extent[0] as f64, extent[1] as f64]);
        if !range.iter().all(|x| x.is_finite()) {
            return Vec::new();
        }
        if range[1] < range[0] {
            range.swap(0, 1);
        }
        let pixels = pixels.clamp(1, 16_384);
        let bin_width = ((range[1] - range[0]) / pixels as f64).max(f64::EPSILON);
        let mut result = Vec::new();
        let mut builder: Option<SegmentBuilder> = None;
        for snapshot in &self.snapshots {
            let value = snapshot.values.get(name);
            if (value.is_none()
                || builder
                    .as_ref()
                    .is_some_and(|builder| builder.segment.session != snapshot.session))
                && let Some(builder) = builder.take()
            {
                builder.finish(&mut result);
            }
            if let Some(&value) = value {
                let builder = builder.get_or_insert_with(|| {
                    SegmentBuilder::new(
                        snapshot.session,
                        self.sessions[&snapshot.session].clone(),
                        range,
                        bin_width,
                    )
                });
                builder.push(Point {
                    tick: snapshot.tick,
                    value,
                    window: snapshot.window,
                });
            }
        }
        if let Some(builder) = builder {
            builder.finish(&mut result);
        }
        result
    }
}

/// Preserve extrema and endpoints in each horizontal display bin.
struct Envelope {
    first: Point,
    min: Point,
    max: Point,
    last: Point,
}

impl Envelope {
    fn new(point: Point) -> Self {
        Self {
            first: point,
            min: point,
            max: point,
            last: point,
        }
    }

    fn push(&mut self, point: Point) {
        if point.value < self.min.value {
            self.min = point;
        }
        if point.value > self.max.value {
            self.max = point;
        }
        self.last = point;
    }

    fn points(&self) -> Vec<Point> {
        let mut points = vec![self.first, self.min, self.max, self.last];
        points.sort_unstable_by_key(|point| point.tick);
        points.dedup_by_key(|point| point.tick);
        points
    }
}

struct SegmentBuilder {
    segment: Segment,
    range: [f64; 2],
    bin_width: f64,
    current: Option<(i64, Envelope)>,
    left: Option<Point>,
    entered: bool,
    exited: bool,
}

impl SegmentBuilder {
    fn new(session: u64, name: String, range: [f64; 2], bin_width: f64) -> Self {
        Self {
            segment: Segment {
                session,
                name,
                points: Vec::new(),
            },
            range,
            bin_width,
            current: None,
            left: None,
            entered: false,
            exited: false,
        }
    }

    fn push(&mut self, point: Point) {
        let tick = point.tick as f64;
        if tick < self.range[0] {
            self.left = Some(point);
            return;
        }
        if !self.entered && tick > self.range[1] && self.left.is_none() {
            return;
        }
        if !self.entered {
            if let Some(left) = self.left.take() {
                self.segment.points.push(left);
            }
            self.entered = true;
        }
        if tick > self.range[1] {
            if !self.exited {
                self.flush();
                self.segment.points.push(point);
                self.exited = true;
            }
            return;
        }
        let bin = ((tick - self.range[0]) / self.bin_width).floor() as i64;
        if let Some((current_bin, envelope)) = &mut self.current
            && *current_bin == bin
        {
            envelope.push(point);
        } else {
            self.flush();
            self.current = Some((bin, Envelope::new(point)));
        }
    }

    fn flush(&mut self) {
        if let Some((_, envelope)) = self.current.take() {
            self.segment.points.extend(envelope.points());
        }
    }

    fn finish(mut self, result: &mut Vec<Segment>) {
        self.flush();
        // A segment entirely outside the viewport does not affect its Y bounds.
        if self.entered && !self.segment.points.is_empty() {
            result.push(self.segment);
        }
    }
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value
        .as_map()?
        .iter()
        .find_map(|(key, value)| (key.as_text() == Some(name)).then_some(value))
}

fn window(value: &Value) -> Option<Window> {
    let value = field(value, "window")?;
    let integer = |name| field(value, name)?.as_integer()?.try_into().ok();
    Some(Window {
        start_tick: integer("start_tick")?,
        end_tick: integer("end_tick")?,
        samples: integer("samples")?,
        complete: field(value, "complete")?.as_bool()?,
    })
}

fn collect_scalars(value: &Value, path: &str, values: &mut BTreeMap<String, f64>) {
    match value {
        Value::Map(entries) => {
            for (key, value) in entries {
                if let Some(key) = key.as_text() {
                    if path.is_empty() && key == "window" {
                        continue;
                    }
                    let child = if path.is_empty() {
                        key.to_owned()
                    } else {
                        format!("{path}.{key}")
                    };
                    collect_scalars(value, &child, values);
                }
            }
        }
        Value::Integer(number) => {
            values.insert(path.to_owned(), i128::from(*number) as f64);
        }
        Value::Float(number) if number.is_finite() => {
            values.insert(path.to_owned(), *number);
        }
        _ => {}
    }
}

#[cfg(test)]
pub(super) mod tests;
