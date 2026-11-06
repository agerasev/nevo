use super::*;
use crate::context::Cpu;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

type Cx = Cpu<Xoshiro256PlusPlus>;

#[test]
fn preparing_and_discarding_memory_does_not_change_the_mind() {
    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    genome.weights[32] = 1.0;
    let mind = Rnn::new(genome).unwrap();
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    let (_, discarded) = mind.compute_update([0.0; 3], &mut cx).unwrap();
    assert_ne!(discarded.memory, mind.memory);
    drop(discarded);
    assert_eq!(mind, original);
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

    assert!(mind.process([f32::NAN, 0.0, 0.0], &mut cx).is_err());
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);
}

#[test]
fn recurrent_units_read_previous_memory_at_a_nondefault_width() {
    let mut genome = RnnGenome {
        hidden_units: 2,
        weights: vec![0.0; 21],
    };
    genome.weights[0] = 1.0; // input 0 -> hidden 0
    genome.weights[8] = 1.0; // previous hidden 0 -> hidden 1
    genome.weights[12] = 1.0; // hidden 0 -> output 0
    genome.weights[15] = 1.0; // hidden 1 -> output 1
    genome.weights[20] = -1.0; // output 2 bias
    let mut brain = Rnn::new(genome).unwrap();
    let mut cx = Cx {
        rng: Xoshiro256PlusPlus::seed_from_u64(73),
    };
    let before = cx.clone();
    assert_eq!(
        brain.process([1.0, 0.0, 0.0], &mut cx).unwrap(),
        [1.0_f32.tanh(), 0.0, -1.0]
    );
    assert_eq!(
        brain.process([0.0; 3], &mut cx).unwrap(),
        [0.0, 1.0_f32.tanh().tanh(), -1.0]
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
    assert_eq!(genome.weights.len(), 101);
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
    assert!(brain.compute_update([0.0; 3], &mut cx).is_err());
    assert_eq!(cx, before);
}
