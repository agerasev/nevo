use super::*;

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-10, "{actual} != {expected}");
}

#[test]
fn selected_reductions_merge_samples_instead_of_averaging_averages() {
    let mut whole = SelectedStats::new(Selection::all());
    let mut first = SelectedStats::new(Selection::all());
    let mut second = SelectedStats::new(Selection::all());
    for value in [1.0, 2.0, 5.0, 8.0] {
        whole.push(value);
    }
    first.push(1.0);
    for value in [2.0, 5.0, 8.0] {
        second.push(value);
    }
    first.merge(&second);
    let mut from_empty = SelectedStats::default();
    from_empty.merge(&first);
    assert_eq!(whole.value(), from_empty.value());
    assert_eq!(
        whole.value(),
        Statistics {
            count: Some(4),
            sum: Some(16.0),
            mean: Some(4.0),
            min: Some(1.0),
            max: Some(8.0),
            variance: Some(7.5),
        }
    );
}

#[test]
fn empty_and_disabled_reductions_have_explicit_semantics() {
    assert_eq!(
        SelectedStats::new(Selection::all()).value(),
        Statistics {
            count: Some(0),
            sum: Some(0.0),
            ..Statistics::default()
        }
    );
    let mut disabled = SelectedStats::default();
    disabled.push(f64::NAN);
    assert_eq!(
        disabled,
        SelectedStats::default(),
        "disabled reducers do no work"
    );

    let mut minimum = SelectedStats::new(Selection {
        min: true,
        ..Selection::default()
    });
    minimum.push(7.0);
    minimum.push(-2.0);
    assert_eq!(
        minimum.value(),
        Statistics {
            min: Some(-2.0),
            ..Statistics::default()
        }
    );
}

#[test]
fn moments_are_stable_for_large_offsets_and_merge_empty_sides() {
    let mut first = Moments::default();
    let mut second = Moments::default();
    for value in [1e12 + 1.0, 1e12 + 2.0] {
        first.push(value);
    }
    for value in [1e12 + 3.0, 1e12 + 4.0] {
        second.push(value);
    }
    first.merge(&second);
    let before = first;
    first.merge(&Moments::default());
    assert_eq!(first, before);
    let mut empty = Moments::default();
    empty.merge(&first);
    assert_eq!(empty, first);
    assert_eq!(first.count(), 4);
    close(first.mean().unwrap(), 1e12 + 2.5);
    close(first.variance().unwrap(), 1.25);
    assert_eq!(Moments::default().variance(), None);
}

#[test]
fn finalized_values_omit_unrequested_fields() {
    let mut stats = SelectedStats::new(Selection {
        mean: true,
        ..Selection::default()
    });
    stats.push(1.0);
    stats.push(3.0);
    let encoded = toml::to_string(&stats.value()).unwrap();
    assert_eq!(encoded, "mean = 2.0\n");
}
