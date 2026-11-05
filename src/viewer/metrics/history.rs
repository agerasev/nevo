use std::collections::{BTreeMap, VecDeque};

use ciborium::Value;

use crate::metrics::log::{Record, Sample};

/// Plotting history is a bounded read cache; the file retains every record.
pub(super) const WINDOW_LIMIT: usize = 8192;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Window {
    pub start_tick: u64,
    pub end_tick: u64,
    pub samples: u64,
    pub complete: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Point {
    pub tick: u64,
    pub value: f64,
    pub window: Option<Window>,
}

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

#[derive(Default)]
pub(super) struct History {
    snapshots: VecDeque<Snapshot>,
    names: BTreeMap<String, usize>,
    sessions: BTreeMap<u64, String>,
    pub total_windows: u64,
}

impl History {
    pub fn ingest(&mut self, record: Record) {
        match record {
            Record::Session(session) => {
                self.sessions.insert(
                    session.id,
                    format!("Session {} · from tick {}", session.id, session.start_tick),
                );
                // Metadata for empty invocations must not grow without bound either.
                while self.sessions.len() > WINDOW_LIMIT {
                    self.sessions.pop_first();
                }
            }
            Record::Sample(sample) => self.push(sample),
        }
    }

    fn push(&mut self, sample: Sample) {
        let mut values = BTreeMap::new();
        collect_scalars(&sample.values, "", &mut values);
        for name in values.keys() {
            *self.names.entry(name.clone()).or_default() += 1;
        }
        let window = field(&sample.values, "window").and_then(|value| {
            Some(Window {
                start_tick: integer(field(value, "start_tick")?)?,
                end_tick: integer(field(value, "end_tick")?)?,
                samples: integer(field(value, "samples")?)?,
                complete: field(value, "complete")?.as_bool()?,
            })
        });
        self.snapshots.push_back(Snapshot {
            session: sample.session,
            tick: sample.tick,
            values,
            window,
        });
        self.total_windows += 1;
        if self.snapshots.len() > WINDOW_LIMIT {
            let expired = self.snapshots.pop_front().unwrap();
            for name in expired.values.keys() {
                let count = self.names.get_mut(name).unwrap();
                *count -= 1;
                if *count == 0 {
                    self.names.remove(name);
                }
            }
        }
        if let Some(first) = self.snapshots.front() {
            self.sessions.retain(|id, _| *id >= first.session);
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn last_tick(&self) -> Option<u64> {
        self.snapshots.back().map(|snapshot| snapshot.tick)
    }

    /// A missing value breaks the curve. Never join separate invocations, even
    /// when their tick ranges overlap after restoring an earlier checkpoint.
    pub fn segments(&self, name: &str) -> Vec<Segment> {
        let mut result: Vec<Segment> = Vec::new();
        let mut previous = None;
        for snapshot in &self.snapshots {
            let Some(&value) = snapshot.values.get(name) else {
                previous = None;
                continue;
            };
            if previous != Some(snapshot.session) {
                result.push(Segment {
                    session: snapshot.session,
                    name: self
                        .sessions
                        .get(&snapshot.session)
                        .cloned()
                        .unwrap_or_else(|| format!("Session {}", snapshot.session)),
                    points: Vec::new(),
                });
            }
            result.last_mut().unwrap().points.push(Point {
                tick: snapshot.tick,
                value,
                window: snapshot.window,
            });
            previous = Some(snapshot.session);
        }
        result
    }
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    value
        .as_map()?
        .iter()
        .find_map(|(key, value)| (key.as_text() == Some(name)).then_some(value))
}

fn integer(value: &Value) -> Option<u64> {
    value.as_integer()?.try_into().ok()
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
        // Summary arrays and booleans are not scalar metric series.
        _ => {}
    }
}
