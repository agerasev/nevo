use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use ciborium::Value;

use super::*;
use crate::metrics::log::{FORMAT_VERSION, Record, Sample, Session};

fn session(id: u64, start_tick: u64) -> Record {
    Record::Session(Session {
        version: FORMAT_VERSION,
        id,
        start_tick,
        started_at: "2026-11-05T00:00:00Z".into(),
        config: "seed = 1\n".into(),
        code_version: "test".into(),
        world_model: "test-world".into(),
        mind_model: "test-mind".into(),
    })
}

fn sample(id: u64, tick: u64, value: Option<f64>, complete: bool) -> Record {
    Record::Sample(Sample {
        session: id,
        tick,
        values: Value::serialized(&serde_json::json!({
            "window": { "start_tick": tick - 1, "end_tick": tick, "samples": 1, "complete": complete },
            "population": { "animals": value },
        })).unwrap(),
    })
}

#[test]
fn gaps_and_overlapping_resumes_are_separate_curves() {
    let mut history = History::default();
    history.ingest(session(1, 0));
    history.ingest(sample(1, 1, Some(10.0), true));
    history.ingest(sample(1, 2, None, true));
    history.ingest(sample(1, 3, Some(20.0), false));
    history.ingest(session(2, 0));
    history.ingest(sample(2, 1, Some(30.0), true));
    assert_eq!(history.names().collect::<Vec<_>>(), ["population.animals"]);
    let segments = history.segments("population.animals");
    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.session)
            .collect::<Vec<_>>(),
        [1, 1, 2]
    );
    assert_eq!(segments[0].points[0].value, 10.0);
    assert_eq!(segments[2].points[0].tick, 1);
    assert_eq!(segments[2].points[0].value, 30.0);
    assert!(!segments[1].points[0].window.unwrap().complete);
    assert!(segments[0].name.contains("from tick 0"));
}

#[test]
fn plot_history_is_bounded_and_expired_metrics_disappear() {
    let mut history = History::default();
    history.ingest(session(1, 0));
    let first = Record::Sample(Sample {
        session: 1,
        tick: 1,
        values: Value::serialized(&serde_json::json!({"old": 4.0})).unwrap(),
    });
    history.ingest(first);
    for tick in 2..=WINDOW_LIMIT as u64 + 1 {
        history.ingest(sample(1, tick, Some(tick as f64), true));
    }
    assert_eq!(history.len(), WINDOW_LIMIT);
    assert_eq!(history.total_windows, WINDOW_LIMIT as u64 + 1);
    assert_eq!(history.names().collect::<Vec<_>>(), ["population.animals"]);
    assert_eq!(history.segments("population.animals")[0].points[0].tick, 2);
}

struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nevo-metrics-panel-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("metrics.cborseq")
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn append(path: &PathBuf, record: &Record) {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    ciborium::into_writer(record, &mut file).unwrap();
    file.flush().unwrap();
}
fn poll(panel: &mut Panel) {
    panel.next_poll = Instant::now();
    panel.poll();
    assert!(panel.error.is_none(), "{:?}", panel.error);
}

#[test]
fn panel_uses_one_reader_before_creation_after_eof_and_through_partial_appends() {
    let temporary = Temporary::new();
    let path = temporary.path();
    let mut panel = Panel::new(Some(path.clone()));
    poll(&mut panel);
    assert_eq!(panel.history.len(), 0);
    append(&path, &session(1, 0));
    append(&path, &sample(1, 1, Some(10.0), true));
    poll(&mut panel);
    assert_eq!(panel.history.len(), 1);
    assert_eq!(panel.selected, "population.animals");
    poll(&mut panel); // EOF is only a temporary lack of new records.
    let record = sample(1, 2, Some(20.0), false);
    let mut encoded = Vec::new();
    ciborium::into_writer(&record, &mut encoded).unwrap();
    let split = encoded.len() / 2;
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(&encoded[..split]).unwrap();
    poll(&mut panel);
    assert_eq!(panel.history.len(), 1);
    assert!(panel.reader.as_ref().unwrap().has_pending_record());
    file.write_all(&encoded[split..]).unwrap();
    poll(&mut panel);
    assert_eq!(panel.history.len(), 2);
    append(&path, &session(2, 0));
    append(&path, &sample(2, 1, Some(30.0), true));
    poll(&mut panel);
    assert_eq!(panel.history.len(), 3);
    assert_eq!(panel.history.segments("population.animals").len(), 2);

    let context = egui::Context::default();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| panel.show(ui));
    output.textures_delta.clear();
    assert!(
        !output.shapes.is_empty(),
        "metrics panel must render its plot and controls"
    );
}

#[test]
fn reader_errors_are_visible_without_discarding_existing_history() {
    let temporary = Temporary::new();
    let path = temporary.path();
    append(&path, &session(1, 0));
    append(&path, &sample(1, 1, Some(10.0), true));
    let mut panel = Panel::new(Some(path.clone()));
    poll(&mut panel);
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    // A standalone break marker is invalid CBOR, not a partial record.
    file.write_all(&[0xff]).unwrap();
    panel.next_poll = Instant::now();
    panel.poll();
    assert!(panel.error.is_some());
    assert_eq!(panel.history.len(), 1);
    let context = egui::Context::default();
    let mut output = context.run_ui(egui::RawInput::default(), |ui| panel.show(ui));
    output.textures_delta.clear();
    assert!(!output.shapes.is_empty());
}
