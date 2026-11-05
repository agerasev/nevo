use std::{
    fs::{self, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
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

fn wait_for(panel: &mut Panel, ready: impl Fn(&Panel) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        panel.poll();
        if ready(panel) {
            return;
        }
        assert!(Instant::now() < deadline, "metrics worker did not finish");
        thread::sleep(Duration::from_millis(5));
    }
}

fn frame(panel: &mut Panel, context: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        events,
        ..Default::default()
    };
    let mut output = context.run_ui(input, |ui| panel.show(ui));
    output.textures_delta.clear();
    assert!(!output.shapes.is_empty());
}

#[test]
fn panel_follows_creation_eof_partial_records_and_resumed_sessions() {
    let temporary = Temporary::new();
    let path = temporary.path();
    let mut panel = Panel::new(Some(path.clone()));
    wait_for(&mut panel, |panel| panel.snapshot.revision > 0);
    assert_eq!(panel.snapshot.total_windows, 0);
    append(&path, &session(1, 0));
    append(&path, &sample(1, 1, Some(10.0), true));
    wait_for(&mut panel, |panel| !panel.snapshot.segments.is_empty());
    assert_eq!(panel.selected, "population.animals");
    let record = sample(1, 2, Some(20.0), false);
    let mut encoded = Vec::new();
    ciborium::into_writer(&record, &mut encoded).unwrap();
    let split = encoded.len() / 2;
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(&encoded[..split]).unwrap();
    wait_for(&mut panel, |panel| panel.snapshot.pending);
    assert_eq!(panel.snapshot.total_windows, 1);
    file.write_all(&encoded[split..]).unwrap();
    wait_for(&mut panel, |panel| panel.snapshot.total_windows == 2);
    append(&path, &session(2, 0));
    append(&path, &sample(2, 1, Some(30.0), true));
    wait_for(&mut panel, |panel| panel.snapshot.total_windows == 3);
    assert_eq!(panel.snapshot.segments.len(), 2);
    assert!(panel.snapshot.error.is_none());
    frame(&mut panel, &egui::Context::default(), Vec::new());
}

#[test]
fn viewport_zoom_requests_detail_and_stays_fixed_while_data_arrives() {
    let temporary = Temporary::new();
    let path = temporary.path();
    append(&path, &session(1, 0));
    for tick in 1..=500 {
        append(&path, &sample(1, tick, Some((tick % 17) as f64), true));
    }
    let mut panel = Panel::new(Some(path.clone()));
    wait_for(&mut panel, |panel| {
        panel.snapshot.total_windows == 500 && !panel.snapshot.segments.is_empty()
    });
    let context = egui::Context::default();
    frame(&mut panel, &context, Vec::new());
    let center = panel.plot_rect.unwrap().center();
    frame(
        &mut panel,
        &context,
        vec![egui::Event::PointerMoved(center), egui::Event::Zoom(8.0)],
    );
    assert!(!panel.follow, "plot navigation must disable follow");
    let zoomed = panel.range.unwrap();
    assert!(zoomed[1] - zoomed[0] < 100.0);
    wait_for(&mut panel, |panel| {
        panel.snapshot.request.range == Some(zoomed)
    });
    let points: Vec<_> = panel
        .snapshot
        .segments
        .iter()
        .flat_map(|segment| &segment.points)
        .collect();
    assert!(!points.is_empty());
    assert!(points.len() < 100);
    frame(&mut panel, &context, Vec::new());
    assert_eq!(panel.range, Some(zoomed), "detail must not refit the plot");

    append(&path, &sample(1, 501, Some(1000.0), true));
    wait_for(&mut panel, |panel| panel.snapshot.total_windows == 501);
    frame(&mut panel, &context, Vec::new());
    assert_eq!(
        panel.range,
        Some(zoomed),
        "new data must not move browsing bounds"
    );

    panel.fit();
    wait_for(&mut panel, |panel| panel.snapshot.request.range.is_none());
    frame(&mut panel, &context, Vec::new());
    let fitted = panel.range.unwrap();
    assert!(fitted[0] <= 1.0 && fitted[1] >= 501.0);
    assert!(
        !panel.follow,
        "fitting must not silently turn follow back on"
    );
}

#[test]
fn reader_errors_are_visible_without_discarding_existing_history() {
    let temporary = Temporary::new();
    let path = temporary.path();
    append(&path, &session(1, 0));
    append(&path, &sample(1, 1, Some(10.0), true));
    let mut panel = Panel::new(Some(path.clone()));
    wait_for(&mut panel, |panel| !panel.snapshot.segments.is_empty());
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(&[0xff]).unwrap();
    wait_for(&mut panel, |panel| panel.snapshot.error.is_some());
    assert_eq!(panel.snapshot.total_windows, 1);
    assert_eq!(panel.snapshot.segments.len(), 1);
    frame(&mut panel, &egui::Context::default(), Vec::new());
}

#[test]
fn following_keeps_full_extent_for_a_metric_absent_from_newer_windows() {
    let temporary = Temporary::new();
    let path = temporary.path();
    append(&path, &session(1, 0));
    append(&path, &sample(1, 1, Some(10.0), true));
    append(&path, &sample(1, 500, None, true));
    let mut panel = Panel::new(Some(path));
    wait_for(&mut panel, |panel| !panel.snapshot.segments.is_empty());
    let context = egui::Context::default();
    for _ in 0..3 {
        frame(&mut panel, &context, Vec::new());
        let bounds = panel.plot_bounds.unwrap();
        assert!(bounds[0] <= 1.0 && bounds[1] >= 500.0);
    }
}

#[test]
fn historical_zoom_uses_loaded_values_without_rereading_old_records() {
    let temporary = Temporary::new();
    let path = temporary.path();
    append(&path, &session(1, 0));
    let first_offset = fs::metadata(&path).unwrap().len();
    for tick in 1..=2400 {
        append(&path, &sample(1, tick, Some(tick as f64), true));
    }
    let mut panel = Panel::new(Some(path.clone()));
    wait_for(&mut panel, |panel| {
        panel.snapshot.total_windows == 2400 && !panel.snapshot.segments.is_empty()
    });
    // Alter already loaded bytes without appending. Browsing should use the
    // retained values; the follower only reads newly appended records.
    let mut file = OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(SeekFrom::Start(first_offset)).unwrap();
    file.write_all(&[0xff]).unwrap();
    panel.follow = false;
    panel.range = Some([1.0, 10.0]);
    panel.request();
    wait_for(&mut panel, |panel| {
        panel.snapshot.request.range == Some([1.0, 10.0])
    });
    assert!(panel.snapshot.error.is_none());
    let points = &panel.snapshot.segments[0].points;
    assert_eq!(points[0].tick, 1);
    assert_eq!(points[0].value, 1.0);
    assert_eq!(points.len(), 11); // Includes the next point for a crossing line.
}
