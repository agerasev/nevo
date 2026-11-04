use super::*;
use crate::context::Cpu;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn recurrent_units_read_previous_memory() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = 1.0; // input 0 -> hidden 0
    genome.weights[16] = 1.0; // previous hidden 0 -> hidden 1
    genome.weights[32] = 1.0; // hidden 0 -> output 0
    genome.weights[37] = 1.0; // hidden 1 -> output 1
    genome.weights[46] = -1.0; // output 2 bias
    let mut mind = Rnn::new(genome);
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(1),
    };
    let before = cx.clone();

    assert_eq!(
        mind.process([1.0, 0.0, 0.0], &Config::default(), &mut cx)
            .unwrap(),
        [1.0_f32.tanh(), 0.0, -1.0]
    );
    assert_eq!(
        mind.process([0.0; 3], &Config::default(), &mut cx).unwrap(),
        [0.0, 1.0_f32.tanh().tanh(), -1.0]
    );
    assert_eq!(mind.memory, [0.0, 1.0_f32.tanh().tanh(), 0.0, 0.0]);
    assert_eq!(cx, before, "processing must not consume genetic draws");
}

#[test]
fn preparing_and_discarding_memory_does_not_change_the_mind() {
    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    genome.weights[32] = 1.0;
    let mut mind = Rnn::new(genome);
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    let (output, discarded) = mind
        .compute_update([0.0; 3], &Config::default(), &mut cx)
        .unwrap();
    assert_eq!(output, [0.5_f32.tanh(), 0.0, 0.0]);
    assert_ne!(discarded.memory, mind.memory);
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);

    let (repeated_output, update) = mind
        .compute_update([0.0; 3], &Config::default(), &mut cx)
        .unwrap();
    assert_eq!((repeated_output, update), (output, discarded));
    <Rnn as Mind<Cpu<Xoshiro256PlusPlus>>>::apply_update(&mut mind, update);
    assert_eq!(mind.memory, update.memory);
    assert_eq!(mind.genome, original.genome);
    assert_eq!(cx, original_cx);
}

#[test]
fn invalid_prepared_memory_fails_before_changing_the_mind() {
    let mut mind = Rnn::new(RnnGenome::default());
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    assert!(
        mind.compute_update([f32::NAN, 0.0, 0.0], &Config::default(), &mut cx)
            .unwrap_err()
            .to_string()
            .contains("prepared recurrent memory")
    );
    assert_eq!(mind, original);
    assert!(
        mind.process([f32::NAN, 0.0, 0.0], &Config::default(), &mut cx)
            .is_err()
    );
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);
}

#[test]
fn metrics_follow_applied_memory_and_restart_after_deserialization() {
    use crate::metrics::Selection;

    let mut genome = RnnGenome::default();
    genome.weights[28] = 3.0;
    let mut mind = Rnn::new(genome);
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let config = Config {
        metrics: MetricsConfig {
            activation: Selection::all(),
            saturation: true,
        },
    };
    let before = mind.metrics();
    let (_, update) = mind.compute_update([0.0; 3], &config, &mut cx).unwrap();
    assert_eq!(mind.metrics(), before, "computing must not publish metrics");
    <Rnn as Mind<Cpu<Xoshiro256PlusPlus>>>::apply_update(&mut mind, update);
    let first_metrics = mind.metrics();
    assert_eq!(first_metrics.value().activation.unwrap().count, Some(4));
    assert_eq!(first_metrics.value().saturation_fraction, Some(0.25));
    assert_eq!(mind.metrics(), first_metrics, "queries are pure");
    assert!(
        mind.compute_update([f32::NAN, 0.0, 0.0], &config, &mut cx)
            .is_err()
    );
    assert_eq!(
        mind.metrics(),
        first_metrics,
        "failed updates preserve caches"
    );

    let mut encoded = Vec::new();
    ciborium::into_writer(&mind, &mut encoded).unwrap();
    let restored: Rnn = ciborium::from_reader(encoded.as_slice()).unwrap();
    assert_eq!(mind, restored, "cache state is outside simulation equality");
    assert_eq!(restored.metrics(), Summary::default());
    let mut reencoded = Vec::new();
    ciborium::into_writer(&restored, &mut reencoded).unwrap();
    assert_eq!(encoded, reencoded, "metrics never enter checkpoint bytes");

    mind.process([0.0; 3], &config, &mut cx).unwrap();
    assert_eq!(
        mind.metrics().value().activation.unwrap().count,
        Some(4),
        "activations describe current units, not a temporal accumulator"
    );
    mind.process([0.0; 3], &Config::default(), &mut cx).unwrap();
    assert_eq!(mind.metrics(), Summary::default());
}

#[test]
fn activation_reductions_merge_across_brains_and_genetic_variance_across_genomes() {
    use crate::metrics::Selection;

    let config = Config {
        metrics: MetricsConfig {
            activation: Selection::all(),
            saturation: true,
        },
    };
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let mut first = Rnn::new(RnnGenome { weights: [0.0; 47] });
    let mut second = Rnn::new(RnnGenome { weights: [1.0; 47] });
    first.process([0.0; 3], &config, &mut cx).unwrap();
    second.process([0.0; 3], &config, &mut cx).unwrap();
    let mut summary = first.metrics();
    second.collect_metrics(&mut summary);
    assert_eq!(summary.value().activation.unwrap().count, Some(8));
    assert_eq!(
        summary.value().activation.unwrap().mean,
        Some(f64::from(1.0_f32.tanh()) / 2.0)
    );

    let mut genomes = first.genome.metrics();
    second.genome.collect_metrics(&mut genomes);
    assert_eq!(genomes.value().genomes, 2);
    assert_eq!(genomes.value().mean_coordinate_variance, Some(0.25));
    let mut varied_coordinates = RnnGenome::default();
    for (index, weight) in varied_coordinates.weights.iter_mut().enumerate() {
        *weight = index as f32;
    }
    assert_eq!(
        varied_coordinates
            .metrics()
            .value()
            .mean_coordinate_variance,
        Some(0.0),
        "one genome has no population diversity regardless of its weight distribution"
    );
    assert_eq!(
        GenomeSummary::default().value().mean_coordinate_variance,
        None
    );
}
