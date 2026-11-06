use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;
use crate::metrics::log::{FORMAT_VERSION, Session};

struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nevo-history-memory-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.join("metrics.cborseq"))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

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

fn sample(id: u64, tick: u64, value: Option<f64>) -> Record {
    Record::Sample(Sample {
        session: id,
        tick,
        values: Value::serialized(&serde_json::json!({
            "window": { "start_tick": tick - 1, "end_tick": tick, "samples": 1, "complete": tick != 3 },
            "value": value,
        })).unwrap(),
    })
}

fn write(path: &Path, records: impl IntoIterator<Item = Record>) {
    let mut file = BufWriter::new(File::create(path).unwrap());
    for record in records {
        ciborium::into_writer(&record, &mut file).unwrap();
    }
    file.flush().unwrap();
}

fn append(path: &Path, record: &Record) {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    ciborium::into_writer(record, &mut file).unwrap();
    file.flush().unwrap();
}

fn catch_up(history: &mut History) {
    while history.poll().unwrap() {}
}

#[test]
fn all_history_remains_in_memory_without_rereading_the_file() {
    let temp = Temporary::new();
    let old = Record::Sample(Sample {
        session: 1,
        tick: 1,
        values: Value::serialized(&serde_json::json!({"old": 42})).unwrap(),
    });
    write(
        &temp.0,
        [session(1, 0), old]
            .into_iter()
            .chain((2..=20_000).map(|tick| sample(1, tick, Some(tick as f64)))),
    );
    let mut history = History::new(temp.0.clone());
    catch_up(&mut history);
    assert_eq!(history.total_windows(), 20_000);
    assert_eq!(history.extent(), Some([1, 20_000]));
    assert_eq!(history.names().collect::<Vec<_>>(), ["old", "value"]);

    // Every metric and every original window remains available even when the
    // source file is gone; plotting must never reread an already loaded record.
    fs::remove_file(&temp.0).unwrap();
    let old = history.query("old", Some([0.0, 5.0]), 800);
    assert_eq!(old[0].points[0].tick, 1);
    assert_eq!(old[0].points[0].value, 42.0);
    for start in (100..20_000).step_by(300) {
        let plotted = history.query("value", Some([start as f64, start as f64 + 5.0]), 800);
        let exact: Vec<_> = plotted
            .iter()
            .flat_map(|segment| &segment.points)
            .filter(|point| (start..=start + 5).contains(&point.tick))
            .map(|point| point.tick)
            .collect();
        assert_eq!(exact, (start..=start + 5).collect::<Vec<_>>());
    }
    let full = history.query("value", None, 1000);
    assert_eq!(full[0].points.first().unwrap().tick, 2);
    assert_eq!(full[0].points.last().unwrap().tick, 20_000);
    assert!(full[0].points.len() <= 4 * 1001);
}

#[test]
fn coarse_plot_preserves_exact_bin_extrema_and_refines_on_zoom() {
    let temp = Temporary::new();
    let value = |tick: u64| {
        if tick == 29 {
            50_000.0
        } else if tick == 1_057 {
            -70_000.0
        } else {
            ((tick * 137) % 997) as f64 - 400.0
        }
    };
    write(
        &temp.0,
        std::iter::once(session(1, 0))
            .chain((1..=20_000).map(|tick| sample(1, tick, Some(value(tick))))),
    );
    let mut history = History::new(temp.0.clone());
    catch_up(&mut history);
    let segments = history.query("value", None, 37);
    assert_eq!(segments.len(), 1);
    let points = &segments[0].points;
    assert!(points.len() <= 4 * 38);
    assert!(points.windows(2).all(|pair| pair[0].tick < pair[1].tick));
    let width = 19_999.0 / 37.0;
    for bin in 0..=37 {
        let actual: Vec<_> = points
            .iter()
            .filter(|point| ((point.tick as f64 - 1.0) / width).floor() as usize == bin)
            .map(|point| point.value)
            .collect();
        let expected: Vec<_> = (1..=20_000)
            .filter(|&tick| ((tick as f64 - 1.0) / width).floor() as usize == bin)
            .map(value)
            .collect();
        if !expected.is_empty() {
            assert_eq!(
                actual.iter().copied().reduce(f64::min),
                expected.iter().copied().reduce(f64::min)
            );
            assert_eq!(
                actual.iter().copied().reduce(f64::max),
                expected.iter().copied().reduce(f64::max)
            );
        }
    }
    let zoom = history.query("value", Some([20.0, 35.0]), 800);
    assert_eq!(
        zoom[0]
            .points
            .iter()
            .filter(|point| (20..=35).contains(&point.tick))
            .count(),
        16
    );
    assert!(
        zoom[0]
            .points
            .iter()
            .any(|point| point.tick == 29 && point.value == 50_000.0)
    );
}

#[test]
fn missing_windows_and_overlapping_sessions_never_join() {
    let temp = Temporary::new();
    write(
        &temp.0,
        std::iter::once(session(1, 0))
            .chain((1..=260).map(|tick| {
                sample(
                    1,
                    tick,
                    (![128, 129, 257].contains(&tick)).then_some(tick as f64),
                )
            }))
            .chain([session(2, 0), sample(2, 1, Some(99.0))]),
    );
    let mut history = History::new(temp.0.clone());
    catch_up(&mut history);
    let plotted = history.query("value", None, 1);
    assert_eq!(
        plotted
            .iter()
            .map(|segment| segment.session)
            .collect::<Vec<_>>(),
        [1, 1, 1, 2]
    );
    assert_eq!(plotted[0].points.last().unwrap().tick, 127);
    assert_eq!(plotted[1].points.first().unwrap().tick, 130);
    assert_eq!(plotted[1].points.last().unwrap().tick, 256);
    assert_eq!(plotted[2].points.first().unwrap().tick, 258);
    assert!(history.query("value", Some([128.0, 129.0]), 800).is_empty());
    assert!(history.query("value", Some([-10.0, -1.0]), 800).is_empty());
}

#[test]
fn crossing_lines_include_neighbours_and_selected_partial_window_metadata() {
    let temp = Temporary::new();
    write(
        &temp.0,
        [
            session(1, 0),
            sample(1, 3, Some(10.0)),
            sample(1, 100, Some(20.0)),
        ],
    );
    let mut history = History::new(temp.0.clone());
    catch_up(&mut history);
    let plotted = history.query("value", Some([40.0, 60.0]), 800);
    assert_eq!(
        plotted[0]
            .points
            .iter()
            .map(|point| point.tick)
            .collect::<Vec<_>>(),
        [3, 100]
    );
    assert!(!plotted[0].points[0].window.unwrap().complete);
}

#[test]
fn following_appends_to_memory_without_changing_older_values() {
    let temp = Temporary::new();
    let mut history = History::new(temp.0.clone());
    assert!(!history.poll().unwrap());
    append(&temp.0, &session(1, 0));
    append(&temp.0, &sample(1, 1, Some(10.0)));
    catch_up(&mut history);
    assert_eq!(history.query("value", None, 800)[0].points.len(), 1);
    append(&temp.0, &sample(1, 2, Some(20.0)));
    catch_up(&mut history);
    assert_eq!(history.query("value", None, 800)[0].points.len(), 2);
    assert_eq!(
        history.query("value", Some([1.0, 1.0]), 800)[0].points[0].value,
        10.0
    );
    append(&temp.0, &session(2, 0));
    append(&temp.0, &sample(2, 1, Some(30.0)));
    catch_up(&mut history);
    let plotted = history.query("value", None, 800);
    assert_eq!(plotted.len(), 2);
    assert_eq!(history.last_tick(), Some(1));
    assert_eq!(history.extent(), Some([1, 2]));
}

#[test]
fn directory_history_discovers_new_files_and_keeps_overlapping_ticks_separate() {
    use crate::metrics::log::create_invocation;
    let temp = Temporary::new();
    let run = temp.0.parent().unwrap();
    // Existing historical sessions remain readable alongside new files.
    write(&temp.0, [session(1, 0), sample(1, 1, Some(10.0))]);
    let mut history = History::new(run.to_owned());
    catch_up(&mut history);
    assert_eq!(history.total_windows(), 1);
    let (_, mut first) = create_invocation(run).unwrap();
    first.append(&session(1, 0)).unwrap();
    first.append(&sample(1, 1, Some(20.0))).unwrap();
    catch_up(&mut history);
    first.append(&sample(1, 2, Some(21.0))).unwrap();
    catch_up(&mut history);
    let (_, mut second) = create_invocation(run).unwrap();
    second.append(&session(1, 0)).unwrap();
    second.append(&sample(1, 1, Some(30.0))).unwrap();
    catch_up(&mut history);
    let segments = history.query("value", None, 800);
    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments.iter().map(|s| s.session).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(segments[0].points[0].value, 10.0);
    assert_eq!(segments[1].points.len(), 2);
    assert_eq!(segments[2].points[0].value, 30.0);
    assert_eq!(history.extent(), Some([1, 2]));
    assert_eq!(history.total_windows(), 4);
}
