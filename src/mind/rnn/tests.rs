use super::*;
use crate::context::Cpu;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

type Cx = Cpu<Xoshiro256PlusPlus>;

#[test]
fn recurrent_units_read_previous_memory() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = 1.0; // input 0 -> hidden 0
    genome.weights[16] = 1.0; // previous hidden 0 -> hidden 1
    genome.weights[32] = 1.0; // hidden 0 -> output 0
    genome.weights[37] = 1.0; // hidden 1 -> output 1
    genome.weights[46] = -1.0; // output 2 bias
    let mut mind = Rnn::new(genome).unwrap();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(1),
    };
    let before = cx.clone();

    assert_eq!(
        mind.process([1.0, 0.0, 0.0], &mut cx).unwrap(),
        [1.0_f32.tanh(), 0.0, -1.0]
    );
    assert_eq!(
        mind.process([0.0; 3], &mut cx).unwrap(),
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
    let mut mind = Rnn::new(genome).unwrap();
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    let (output, discarded) = mind.compute_update([0.0; 3], &mut cx).unwrap();
    assert_eq!(output, [0.5_f32.tanh(), 0.0, 0.0]);
    assert_ne!(discarded.memory, mind.memory);
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);

    let (repeated_output, update) = mind.compute_update([0.0; 3], &mut cx).unwrap();
    assert_eq!((repeated_output, &update), (output, &discarded));
    <Rnn as Mind<Cpu<Xoshiro256PlusPlus>>>::apply_update(&mut mind, update);
    assert_eq!(mind.memory, discarded.memory);
    assert_eq!(mind.genome, original.genome);
    assert_eq!(cx, original_cx);
}

#[test]
fn invalid_prepared_memory_fails_before_changing_the_mind() {
    let mut mind = Rnn::new(RnnGenome::default()).unwrap();
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    assert!(
        mind.compute_update([f32::NAN, 0.0, 0.0], &mut cx)
            .unwrap_err()
            .to_string()
            .contains("prepared recurrent memory")
    );
    assert_eq!(mind, original);
    assert!(mind.process([f32::NAN, 0.0, 0.0], &mut cx).is_err());
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);
}

#[test]
fn a_two_unit_brain_uses_the_same_recurrent_layout_and_restores_memory() {
    use crate::genome::RandomInit;
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(73),
    };
    let before = cx.clone();
    let mut genome = RnnGenome::random(
        &Initialization {
            hidden_units: 2,
            weight_min: 0.0,
            weight_max: 0.0,
        },
        &mut cx,
    )
    .unwrap();
    assert_eq!(cx, before, "constant initialization consumes no draws");
    assert_eq!(genome.weights.len(), 21);
    genome.weights[0] = 1.0; // input 0 -> hidden 0
    genome.weights[8] = 1.0; // previous hidden 0 -> hidden 1
    genome.weights[12] = 1.0; // hidden 0 -> output 0
    genome.weights[15] = 1.0; // hidden 1 -> output 1
    let mut brain = Rnn::new(genome).unwrap();
    assert_eq!(
        brain.process([1.0, 0.0, 0.0], &mut cx).unwrap(),
        [1.0_f32.tanh(), 0.0, 0.0]
    );
    let mut bytes = Vec::new();
    ciborium::into_writer(&brain, &mut bytes).unwrap();
    let mut restored: Rnn = ciborium::from_reader(bytes.as_slice()).unwrap();
    <Rnn as Mind<Cx>>::configure(&mut restored, &Config::default()).unwrap();
    let expected = [0.0, 1.0_f32.tanh().tanh(), 0.0];
    assert_eq!(brain.process([0.0; 3], &mut cx).unwrap(), expected);
    assert_eq!(restored.process([0.0; 3], &mut cx).unwrap(), expected);
    assert_eq!(restored, brain);
    assert_eq!(cx, before);
}

#[test]
fn initialization_uses_configured_width_range_and_draw_order() {
    use crate::genome::RandomInit;
    use rand::RngExt;
    let config = Initialization {
        hidden_units: 7,
        weight_min: -0.25,
        weight_max: 0.75,
    };
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(19),
    };
    let mut reference = cx.clone();
    let genome = RnnGenome::random(&config, &mut cx).unwrap();
    let expected: Vec<f32> = (0..101)
        .map(|_| reference.rng.random_range(-0.25..0.75))
        .collect();
    assert_eq!(genome.hidden_units, 7);
    assert_eq!(genome.weights, expected);
    assert_eq!(cx, reference);
    let brain = Rnn::new(genome).unwrap();
    assert_eq!(brain.memory, vec![0.0; 7]);
}

#[test]
fn invalid_initialization_and_dynamic_shapes_fail_before_using_randomness() {
    use crate::genome::RandomInit;
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(19),
    };
    let before = cx.clone();
    for config in [
        Initialization {
            hidden_units: 0,
            ..Initialization::default()
        },
        Initialization {
            hidden_units: usize::MAX,
            ..Initialization::default()
        },
        Initialization {
            weight_min: 1.0,
            weight_max: -1.0,
            ..Initialization::default()
        },
        Initialization {
            weight_min: f32::NAN,
            ..Initialization::default()
        },
        Initialization {
            weight_max: f32::INFINITY,
            ..Initialization::default()
        },
        Initialization {
            weight_min: -f32::MAX,
            weight_max: f32::MAX,
            ..Initialization::default()
        },
    ] {
        assert!(RnnGenome::random(&config, &mut cx).is_err());
        assert_eq!(cx, before);
    }
    for hidden_units in [0, 2, usize::MAX] {
        assert!(
            Rnn::new(RnnGenome {
                hidden_units,
                ..RnnGenome::default()
            })
            .is_err()
        );
    }
    let mut brain = Rnn::new(RnnGenome::default()).unwrap();
    brain.memory.pop();
    assert!(brain.compute_update([0.0; 3], &mut cx).is_err());
    assert_eq!(cx, before);
}
