use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

use super::*;
use crate::{
    context::Cpu,
    genome::Mutate,
    mind::{Heredity, rnn::WeightMutation},
};

type Cx = Cpu<Xoshiro256PlusPlus>;

#[test]
fn offspring_inherit_mutated_genomes_and_fresh_memory() {
    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    let heredity = HerbivoreHeredity::with_immigrant_genome(genome.clone());
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut reference_cx = cx.clone();
    let mut parent = heredity.immigrant(&mut cx).unwrap();
    parent.process(&mut cx, FoodObservation::default()).unwrap();
    assert_eq!(cx, reference_cx, "template and inference consume no draws");
    assert_ne!(parent.brain.memory, [0.0; 4]);
    let original = parent.clone();

    let child = heredity.offspring(&mut cx, &parent).unwrap();
    let mut expected = genome.clone();
    expected
        .mutate(&WeightMutation::default(), &mut reference_cx)
        .unwrap();
    assert_eq!(parent, original);
    assert_eq!(child.brain.memory, [0.0; 4]);
    assert_eq!(child.brain.genome, expected);
    assert_ne!(child.brain.genome, genome);
    assert_eq!(cx, reference_cx);
    for (inherited, original) in child.brain.genome.weights.iter().zip(genome.weights) {
        assert!((inherited - original).abs() <= 0.02 + f32::EPSILON);
    }

    let later = heredity.immigrant(&mut cx).unwrap();
    assert_eq!(later.brain.genome, genome);
    assert_eq!(later.brain.memory, [0.0; 4]);
    assert_eq!(cx, reference_cx);
}

#[test]
fn default_heredity_draws_fresh_genomes_for_every_immigrant() {
    let heredity = HerbivoreHeredity::default();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut reference_cx = cx.clone();
    let first = heredity.immigrant(&mut cx).unwrap();
    assert_eq!(first.brain.genome, RnnGenome::random(&mut reference_cx));
    assert_eq!(cx, reference_cx);
    let later = heredity.immigrant(&mut cx).unwrap();
    assert_eq!(later.brain.genome, RnnGenome::random(&mut reference_cx));
    assert_eq!(cx, reference_cx);
    assert_ne!(first.brain.genome, later.brain.genome);
    for mind in [first, later] {
        assert_eq!(mind.brain.memory, [0.0; 4]);
        assert!(
            mind.brain
                .genome
                .weights
                .iter()
                .all(|weight| (-0.5..0.5).contains(weight))
        );
    }
}

#[test]
fn non_finite_brain_state_and_immigrant_template_are_rejected() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = f32::NAN;
    let invalid = HerbivoreMind::new(genome.clone());
    assert!(<HerbivoreMind as Mind<Cx>>::validate(&invalid).is_err());
    let heredity = HerbivoreHeredity::with_immigrant_genome(genome);
    assert!(<HerbivoreHeredity as Heredity<Cx, HerbivoreMind>>::validate(&heredity).is_err());

    let mut mind = HerbivoreMind::new(RnnGenome::default());
    mind.brain.memory[0] = f32::INFINITY;
    assert!(<HerbivoreMind as Mind<Cx>>::validate(&mind).is_err());
}
