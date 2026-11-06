use rand::{RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use super::*;
use crate::{
    context::Cpu,
    genome::{Mutate, RandomInit},
    mind::rnn::WeightMutation,
};

type Cx = Cpu<Xoshiro256PlusPlus>;

#[test]
fn genome_conversion_creates_a_fresh_mind_without_cloning_memory() {
    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut reference_cx = cx.clone();
    let mut parent = AnimalMind::try_from(genome.clone()).unwrap();
    parent.process(FoodObservation::default(), &mut cx).unwrap();
    assert_eq!(
        cx, reference_cx,
        "construction and inference consume no draws"
    );
    assert_ne!(parent.brain.memory, [0.0; 4]);
    let original = parent.clone();

    let mut child_genome = parent.as_ref().clone();
    child_genome
        .mutate(&WeightMutation::default(), &mut cx)
        .unwrap();
    let child = AnimalMind::try_from(child_genome).unwrap();
    let expected_weights = genome
        .weights
        .iter()
        .map(|weight| weight + reference_cx.rng.random_range(-0.02..0.02))
        .collect::<Vec<_>>();
    assert_eq!(parent, original);
    assert_eq!(child.brain.memory, [0.0; 4]);
    assert_eq!(child.as_ref().weights, expected_weights);
    assert_ne!(child.as_ref(), &genome);
    assert_eq!(cx, reference_cx);

    let rebuilt_brain = Rnn::try_from(parent.brain.as_ref().clone()).unwrap();
    assert_eq!(rebuilt_brain.as_ref(), &genome);
    assert_eq!(rebuilt_brain.memory, [0.0; 4]);
}

#[test]
fn random_initialization_preserves_the_expected_draw_sequence() {
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut reference_cx = cx.clone();
    let mut previous = None;
    for _ in 0..2 {
        let genome =
            RnnGenome::random(&crate::mind::rnn::Initialization::default(), &mut cx).unwrap();
        let expected_weights: [f32; 47] =
            std::array::from_fn(|_| reference_cx.rng.random_range(-0.5..0.5));
        assert_eq!(genome.weights, expected_weights);
        assert_eq!(cx, reference_cx);
        if let Some(previous) = previous {
            assert_ne!(genome, previous);
        }
        previous = Some(genome.clone());
        let mind = AnimalMind::try_from(genome).unwrap();
        assert_eq!(mind.brain.memory, [0.0; 4]);
        assert!(
            mind.as_ref()
                .weights
                .iter()
                .all(|weight| (-0.5..0.5).contains(weight))
        );
    }
}

#[test]
fn non_finite_brain_state_is_rejected() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = f32::NAN;
    assert!(AnimalMind::new(genome).is_err());

    let mut mind = AnimalMind::new(RnnGenome::default()).unwrap();
    mind.brain.memory[0] = f32::INFINITY;
    assert!(<AnimalMind as Mind<Cx>>::validate(&mind).is_err());
}
