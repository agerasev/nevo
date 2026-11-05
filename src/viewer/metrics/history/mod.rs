//! Rebuildable block summaries index the whole log; detailed values stay on disk.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
};

use anyhow::{Result, ensure};
use ciborium::Value;

use crate::metrics::log::{LocatedRecord, Reader, Record, Sample, read_range};

const BLOCK_WINDOWS: usize = 128;
const BLOCK_BYTES: u64 = 1024 * 1024;
const CACHE_BLOCKS: usize = 16;
const CACHE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Window {
    pub start_tick: u64,
    pub end_tick: u64,
    pub samples: u64,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug)]
struct IndexedPoint {
    ordinal: u64,
    point: Point,
}

/// Four original samples suffice to merge extrema without losing a spike.
#[derive(Clone)]
struct Envelope {
    first: IndexedPoint,
    min: IndexedPoint,
    max: IndexedPoint,
    last: IndexedPoint,
}

impl Envelope {
    fn new(point: IndexedPoint) -> Self {
        Self {
            first: point.clone(),
            min: point.clone(),
            max: point.clone(),
            last: point,
        }
    }

    fn push(&mut self, point: IndexedPoint) {
        if point.point.value < self.min.point.value {
            self.min = point.clone();
        }
        if point.point.value > self.max.point.value {
            self.max = point.clone();
        }
        self.last = point;
    }

    fn points(&self) -> Vec<IndexedPoint> {
        let mut points = vec![
            self.first.clone(),
            self.min.clone(),
            self.max.clone(),
            self.last.clone(),
        ];
        points.sort_unstable_by_key(|point| point.ordinal);
        points.dedup_by_key(|point| point.ordinal);
        points
    }

    fn span(&self) -> Span {
        Span {
            first: self.first.ordinal,
            last: self.last.ordinal,
            points: self.points(),
        }
    }
}

#[derive(Clone)]
struct Span {
    first: u64,
    last: u64,
    points: Vec<IndexedPoint>,
}

struct Block {
    session: u64,
    offset: u64,
    end_offset: u64,
    first_ordinal: u64,
    windows: usize,
    first_tick: u64,
    last_tick: u64,
    metrics: BTreeMap<String, Vec<Envelope>>,
}

impl Block {
    fn push(&mut self, sample: Sample, end_offset: u64, ordinal: u64) {
        self.end_offset = end_offset;
        self.last_tick = sample.tick;
        self.windows += 1;
        let window = window(&sample.values);
        let mut values = BTreeMap::new();
        collect_scalars(&sample.values, "", &mut values);
        for (name, value) in values {
            let spans = self.metrics.entry(name).or_default();
            let point = IndexedPoint {
                ordinal,
                point: Point {
                    tick: sample.tick,
                    value,
                    window,
                },
            };
            if let Some(span) = spans.last_mut()
                && span.last.ordinal + 1 == ordinal
            {
                span.push(point);
            } else {
                spans.push(Envelope::new(point));
            }
        }
    }
}

struct CachedBlock {
    index: usize,
    end_offset: u64,
    bytes: u64,
    spans: Vec<Span>,
}

pub(super) struct History {
    path: PathBuf,
    reader: Reader,
    blocks: Vec<Block>,
    active: Option<Block>,
    names: BTreeSet<String>,
    sessions: BTreeMap<u64, String>,
    total_windows: u64,
    last_tick: Option<u64>,
    extent: Option<[u64; 2]>,
    cached_metric: String,
    cache: VecDeque<CachedBlock>,
    cache_bytes: u64,
    #[cfg(test)]
    disk_reads: usize,
}

impl History {
    pub fn new(path: PathBuf) -> Self {
        Self {
            reader: Reader::new(&path),
            path,
            blocks: Vec::new(),
            active: None,
            names: BTreeSet::new(),
            sessions: BTreeMap::new(),
            total_windows: 0,
            last_tick: None,
            extent: None,
            cached_metric: String::new(),
            cache: VecDeque::new(),
            cache_bytes: 0,
            #[cfg(test)]
            disk_reads: 0,
        }
    }

    /// One bounded batch. EOF remains temporary, including before file creation.
    pub fn poll(&mut self) -> Result<bool> {
        let records = self.reader.poll_located()?;
        let changed = !records.is_empty();
        for record in records {
            self.ingest(record);
        }
        Ok(changed)
    }

    fn ingest(&mut self, located: LocatedRecord) {
        match located.record {
            Record::Session(session) => {
                if let Some(block) = self.active.take() {
                    self.blocks.push(block);
                }
                self.sessions.insert(
                    session.id,
                    format!("Session {} · from tick {}", session.id, session.start_tick),
                );
            }
            Record::Sample(sample) => {
                let block = self.active.get_or_insert_with(|| Block {
                    session: sample.session,
                    offset: located.offset,
                    end_offset: located.offset,
                    first_ordinal: self.total_windows,
                    windows: 0,
                    first_tick: sample.tick,
                    last_tick: sample.tick,
                    metrics: BTreeMap::new(),
                });
                self.last_tick = Some(sample.tick);
                self.extent = Some(match self.extent {
                    None => [sample.tick, sample.tick],
                    Some([min, max]) => [min.min(sample.tick), max.max(sample.tick)],
                });
                block.push(sample, located.end_offset, self.total_windows);
                for name in block.metrics.keys() {
                    if !self.names.contains(name) {
                        self.names.insert(name.clone());
                    }
                }
                self.total_windows += 1;
                if block.windows >= BLOCK_WINDOWS || block.end_offset - block.offset >= BLOCK_BYTES
                {
                    self.blocks.push(self.active.take().unwrap());
                }
            }
        }
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    pub fn total_windows(&self) -> u64 {
        self.total_windows
    }

    pub fn last_tick(&self) -> Option<u64> {
        self.last_tick
    }

    pub fn extent(&self) -> Option<[u64; 2]> {
        self.extent
    }

    pub fn has_pending_record(&self) -> bool {
        self.reader.has_pending_record()
    }

    fn block(&self, index: usize) -> &Block {
        self.blocks
            .get(index)
            .unwrap_or_else(|| self.active.as_ref().unwrap())
    }

    /// Load only the blocks needing detail. Coarse summaries and a final
    /// first/min/max/last reduction retain the actual extrema in each pixel bin.
    /// Samples adjoining the viewport are included so crossing lines remain visible.
    pub fn query(
        &mut self,
        name: &str,
        range: Option<[f64; 2]>,
        pixels: usize,
    ) -> Result<Vec<Segment>> {
        let Some(extent) = self.extent else {
            return Ok(Vec::new());
        };
        let mut range = range.unwrap_or([extent[0] as f64, extent[1] as f64]);
        ensure!(
            range.iter().all(|x| x.is_finite()),
            "non-finite plot bounds"
        );
        if range[1] < range[0] {
            range.swap(0, 1);
        }
        let pixels = pixels.clamp(1, 16_384);
        let bin_width = ((range[1] - range[0]) / pixels as f64).max(f64::EPSILON);
        if self.cached_metric != name {
            self.cache.clear();
            self.cache_bytes = 0;
            self.cached_metric = name.to_owned();
        }
        let count = self.blocks.len() + usize::from(self.active.is_some());
        let mut result = Vec::new();
        let mut start = 0;
        while start < count {
            let session = self.block(start).session;
            let end = self.partition_blocks(start, count, |block| block.session == session);
            let first =
                self.partition_blocks(start, end, |block| (block.last_tick as f64) < range[0]);
            let last =
                self.partition_blocks(first, end, |block| block.first_tick as f64 <= range[1]);
            let first = first.saturating_sub(1).max(start);
            let last = last.saturating_add(1).min(end);
            let mut builder: Option<SegmentBuilder> = None;
            let mut previous = None;
            for index in first..last {
                let block = self.block(index);
                let bin = |tick: u64| ((tick as f64 - range[0]) / bin_width).floor();
                let use_summary = block.first_tick as f64 >= range[0]
                    && block.last_tick as f64 <= range[1]
                    && bin(block.first_tick) == bin(block.last_tick);
                let spans = if use_summary {
                    block
                        .metrics
                        .get(name)
                        .map_or_else(Vec::new, |spans| spans.iter().map(Envelope::span).collect())
                } else {
                    self.raw_spans(index, name)?
                };
                for span in spans {
                    if previous != span.first.checked_sub(1) {
                        if let Some(builder) = builder.take() {
                            builder.finish(&mut result);
                        }
                        builder = Some(SegmentBuilder::new(
                            session,
                            self.sessions[&session].clone(),
                            range,
                            bin_width,
                        ));
                    }
                    let builder = builder.get_or_insert_with(|| {
                        SegmentBuilder::new(
                            session,
                            self.sessions[&session].clone(),
                            range,
                            bin_width,
                        )
                    });
                    for point in span.points {
                        builder.push(point);
                    }
                    previous = Some(span.last);
                }
            }
            if let Some(builder) = builder {
                builder.finish(&mut result);
            }
            start = end;
        }
        Ok(result)
    }

    fn partition_blocks(
        &self,
        mut start: usize,
        mut end: usize,
        predicate: impl Fn(&Block) -> bool,
    ) -> usize {
        while start < end {
            let mid = start + (end - start) / 2;
            if predicate(self.block(mid)) {
                start = mid + 1;
            } else {
                end = mid;
            }
        }
        start
    }

    fn raw_spans(&mut self, index: usize, name: &str) -> Result<Vec<Span>> {
        let block = self.block(index);
        let (offset, end_offset, ordinal, session, windows) = (
            block.offset,
            block.end_offset,
            block.first_ordinal,
            block.session,
            block.windows,
        );
        if let Some(position) = self
            .cache
            .iter()
            .position(|cached| cached.index == index && cached.end_offset == end_offset)
        {
            let cached = self.cache.remove(position).unwrap();
            let spans = cached.spans.clone();
            self.cache.push_back(cached);
            return Ok(spans);
        }
        #[cfg(test)]
        {
            self.disk_reads += 1;
        }
        let records = read_range(&self.path, offset..end_offset)?;
        ensure!(records.len() == windows, "indexed metrics block changed");
        let mut spans: Vec<Span> = Vec::new();
        for (index, record) in records.into_iter().enumerate() {
            let Record::Sample(sample) = record else {
                anyhow::bail!("indexed metrics block contains unexpected metadata");
            };
            ensure!(sample.session == session, "indexed metrics session changed");
            let mut values = BTreeMap::new();
            collect_scalars(&sample.values, "", &mut values);
            let Some(value) = values.get(name).copied() else {
                continue;
            };
            let ordinal = ordinal + index as u64;
            let point = IndexedPoint {
                ordinal,
                point: Point {
                    tick: sample.tick,
                    value,
                    window: window(&sample.values),
                },
            };
            if let Some(span) = spans.last_mut()
                && span.last + 1 == ordinal
            {
                span.last = ordinal;
                span.points.push(point);
            } else {
                spans.push(Span {
                    first: ordinal,
                    last: ordinal,
                    points: vec![point],
                });
            }
        }
        let bytes = end_offset - offset;
        if bytes <= CACHE_BYTES {
            if let Some(position) = self.cache.iter().position(|cached| cached.index == index) {
                self.cache_bytes -= self.cache.remove(position).unwrap().bytes;
            }
            while self.cache.len() >= CACHE_BLOCKS || self.cache_bytes + bytes > CACHE_BYTES {
                self.cache_bytes -= self.cache.pop_front().unwrap().bytes;
            }
            self.cache_bytes += bytes;
            self.cache.push_back(CachedBlock {
                index,
                end_offset,
                bytes,
                spans: spans.clone(),
            });
        }
        Ok(spans)
    }
}

struct SegmentBuilder {
    segment: Segment,
    range: [f64; 2],
    bin_width: f64,
    current: Option<(i64, Envelope)>,
    left: Option<IndexedPoint>,
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

    fn push(&mut self, point: IndexedPoint) {
        let tick = point.point.tick as f64;
        if tick < self.range[0] {
            self.left = Some(point);
            return;
        }
        if !self.entered && tick > self.range[1] && self.left.is_none() {
            return;
        }
        if !self.entered {
            if let Some(left) = self.left.take() {
                self.segment.points.push(left.point);
            }
            self.entered = true;
        }
        if tick > self.range[1] {
            if !self.exited {
                self.flush();
                self.segment.points.push(point.point);
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
            self.segment
                .points
                .extend(envelope.points().into_iter().map(|point| point.point));
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
mod tests;
