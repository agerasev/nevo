use super::*;

// Shared by history and panel tests; no filesystem or simulation is needed.
pub(in crate::viewer::metrics) fn history(values: &[(u64, u64, Option<f64>)]) -> History {
    let mut history = History::new(PathBuf::new());
    for &(session, tick, value) in values {
        history
            .sessions
            .insert(session, format!("Session {session}"));
        history.push(Sample {
            session,
            tick,
            values: Value::serialized(&serde_json::json!({
                "value": value,
                "window": {"start_tick": tick - 1, "end_tick": tick, "samples": 1, "complete": false},
            })).unwrap(),
        });
    }
    history
}

#[test]
fn downsampling_keeps_spikes_and_endpoints_without_losing_zoom_detail() {
    let mut values: Vec<_> = (1..=17).map(|tick| (1, tick, Some(0.0))).collect();
    values[2].2 = Some(10.0);
    values[4].2 = Some(-10.0);
    let history = history(&values);
    let coarse = history.query("value", None, 1);
    let ticks: Vec<_> = coarse[0].points.iter().map(|p| p.tick).collect();
    assert_eq!(ticks, [1, 3, 5, 16, 17]);
    let zoom = history.query("value", Some([2.0, 6.0]), 100);
    assert_eq!(
        zoom[0].points.iter().map(|p| p.tick).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6, 7]
    );
}

#[test]
fn missing_values_and_overlapping_sessions_break_curves() {
    let history = history(&[
        (1, 1, Some(10.0)),
        (1, 2, None),
        (1, 3, Some(20.0)),
        (2, 1, Some(30.0)),
    ]);
    let plotted = history.query("value", None, 1);
    assert_eq!(
        plotted
            .iter()
            .map(|s| (s.session, s.points[0].value))
            .collect::<Vec<_>>(),
        [(1, 10.0), (1, 20.0), (2, 30.0)]
    );
    assert!(history.query("value", Some([2.0, 2.0]), 100).is_empty());
}

#[test]
fn crossing_lines_keep_neighbours_and_window_metadata() {
    let history = history(&[(1, 3, Some(10.0)), (1, 100, Some(20.0))]);
    let plotted = history.query("value", Some([40.0, 60.0]), 100);
    assert_eq!(
        plotted[0].points.iter().map(|p| p.tick).collect::<Vec<_>>(),
        [3, 100]
    );
    assert_eq!(plotted[0].points[0].window.unwrap().start_tick, 2);
}

#[test]
fn scalar_names_exclude_window_metadata_and_missing_values() {
    let value = Value::serialized(&serde_json::json!({
        "window": {"samples": 5}, "animal": {"energy": 0.5}, "missing": null,
    }))
    .unwrap();
    let mut values = BTreeMap::new();
    collect_scalars(&value, "", &mut values);
    assert_eq!(values, BTreeMap::from([("animal.energy".into(), 0.5)]));
}
