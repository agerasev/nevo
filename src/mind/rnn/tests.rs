use super::*;
use crate::context::Cpu;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

type Cx = Cpu<Xoshiro256PlusPlus>;

#[test]
fn preparing_and_discarding_memory_does_not_change_the_mind() {
    let mut genome = RnnGenome::default();
    genome.weights[40] = 0.5; // hidden 0 bias
    genome.weights[44] = 1.0; // hidden 0 -> output 0
    let mind = Rnn::new(genome).unwrap();
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    let (_, discarded) = mind.compute_update([0.0; INPUTS], &mut cx).unwrap();
    assert_ne!(discarded.memory, mind.memory);
    drop(discarded);
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);
}

#[test]
fn nonfinite_inputs_fail_before_changing_the_mind() {
    let mut mind = Rnn::new(RnnGenome::default()).unwrap();
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut input = [0.0; INPUTS];
        input[0] = value;
        assert!(mind.process(input, &mut cx).is_err());
    }
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);
}

#[test]
fn output_overflow_does_not_publish_prepared_memory() {
    let mut genome = RnnGenome {
        hidden_units: 2,
        weights: vec![0.0; 24],
    };
    genome.weights[16..18].fill(f32::MAX); // Saturate both hidden units.
    genome.weights[18..20].fill(f32::MAX); // Their finite contributions overflow output 0.
    let mut brain = Rnn::new(genome).unwrap();
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(73),
    };
    assert!(brain.process([0.0; INPUTS], &mut cx).is_err());
    assert_eq!(brain.memory, [0.0, 0.0]);
}

#[test]
fn recurrent_units_read_previous_memory_at_a_nondefault_width() {
    let mut genome = RnnGenome {
        hidden_units: 2,
        weights: vec![0.0; 24],
    };
    genome.weights[0] = 1.0; // input 0 -> hidden 0
    genome.weights[14] = 1.0; // previous hidden 0 -> hidden 1
    genome.weights[18] = 1.0; // hidden 0 -> output 0
    genome.weights[21] = 1.0; // hidden 1 -> output 1
    genome.weights[23] = -1.0; // output 1 bias
    let mut brain = Rnn::new(genome).unwrap();
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(73),
    };
    let before = cx.clone();
    let mut input = [0.0; INPUTS];
    input[0] = 1.0;
    assert_eq!(
        brain.process(input, &mut cx).unwrap(),
        [1.0_f32.tanh(), -1.0]
    );
    assert_eq!(
        brain.process([0.0; INPUTS], &mut cx).unwrap(),
        [0.0, 1.0_f32.tanh().tanh() - 1.0]
    );
    assert_eq!(cx, before);
}

#[test]
fn initialization_respects_width_and_range() {
    use crate::genome::RandomInit;
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(19),
    };
    let config = Initialization {
        hidden_units: 7,
        weight_min: -0.25,
        weight_max: 0.75,
    };
    let genome = RnnGenome::random(&config, &mut cx).unwrap();
    assert_eq!(genome.weights.len(), 114);
    assert_eq!(RnnGenome::default().weights.len(), 54);
    assert!(genome.weights.iter().all(|w| (-0.25..0.75).contains(w)));
    assert_eq!(Rnn::new(genome).unwrap().memory, vec![0.0; 7]);
}

#[test]
fn constant_initialization_consumes_no_randomness() {
    use crate::genome::RandomInit;
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(19),
    };
    let before = cx.clone();
    let config = Initialization {
        weight_min: 0.5,
        weight_max: 0.5,
        ..Default::default()
    };
    let genome = RnnGenome::random(&config, &mut cx).unwrap();
    assert!(genome.weights.iter().all(|&w| w == 0.5));
    assert_eq!(cx, before);
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
    assert!(brain.compute_update([0.0; INPUTS], &mut cx).is_err());
    assert_eq!(cx, before);
}
