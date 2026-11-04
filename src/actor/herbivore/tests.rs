use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

use super::*;
use crate::{
    actor::{
        HandleAllocator,
        population::{Lifecycle, PopulationObservation},
    },
    context::Cpu,
    environment::ecosystem::Origin,
    genome::Mutate,
    mind::rnn::WeightMutation,
};

#[test]
fn offspring_mutate_the_genome_and_start_with_fresh_memory() {
    let mut immigrant_genome = RnnGenome::default();
    immigrant_genome.weights[28] = 0.5;
    let mut population = Herbivores::with_immigrant_genome(immigrant_genome.clone());
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut reference_cx = cx.clone();
    let mut allocator = HandleAllocator::new();
    let parent = allocator.allocate().unwrap();
    let child = allocator.allocate().unwrap();
    let immigrant = allocator.allocate().unwrap();
    population
        .observe(
            &mut cx,
            PopulationObservation::Lifecycle {
                members: vec![parent],
                changes: vec![Lifecycle::Spawn {
                    handle: parent,
                    origin: Origin::Immigrant,
                }],
            },
        )
        .unwrap();
    population
        .observe(
            &mut cx,
            PopulationObservation::Inputs {
                members: vec![parent],
                inputs: vec![(parent, FoodObservation::default())],
            },
        )
        .unwrap();
    population.act(&mut cx).unwrap();
    assert_eq!(
        cx, reference_cx,
        "known immigrant genomes and inference consume no draws"
    );
    let original = population.get(parent).unwrap().clone();
    assert_ne!(original.mind.memory, [0.0; 4]);

    population
        .observe(
            &mut cx,
            PopulationObservation::Lifecycle {
                members: vec![parent, child],
                changes: vec![Lifecycle::Spawn {
                    handle: child,
                    origin: Origin::Child { parent },
                }],
            },
        )
        .unwrap();
    let baby = population.get(child).unwrap();
    assert_eq!(population.get(parent), Some(&original));
    assert_eq!(baby.mind.memory, [0.0; 4]);
    let mut expected = immigrant_genome.clone();
    expected
        .mutate(&WeightMutation::default(), &mut reference_cx)
        .unwrap();
    assert_eq!(baby.mind.genome, expected);
    assert_eq!(cx, reference_cx);
    assert_ne!(baby.mind.genome, immigrant_genome);
    for (inherited, original) in baby
        .mind
        .genome
        .weights
        .iter()
        .zip(immigrant_genome.weights)
    {
        assert!((inherited - original).abs() <= 0.02 + f32::EPSILON);
    }
    assert!(
        population.act(&mut cx).is_err(),
        "births do not schedule an action"
    );

    population
        .observe(
            &mut cx,
            PopulationObservation::Lifecycle {
                members: vec![child, immigrant],
                changes: vec![
                    Lifecycle::Spawn {
                        handle: immigrant,
                        origin: Origin::Immigrant,
                    },
                    Lifecycle::Despawn { handle: parent },
                ],
            },
        )
        .unwrap();
    let migrant = population.get(immigrant).unwrap();
    assert_eq!(migrant.mind.genome, immigrant_genome);
    assert_eq!(migrant.mind.memory, [0.0; 4]);
    assert_eq!(cx, reference_cx);
    assert!(population.get(parent).is_none());
    assert!(population.get(child).is_some());
}

#[test]
fn default_factory_draws_fresh_genomes_for_every_immigrant() {
    let mut population = Herbivores::default();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut reference_cx = cx.clone();
    let mut allocator = HandleAllocator::new();
    let first = allocator.allocate().unwrap();
    let later = allocator.allocate().unwrap();
    for (handle, members) in [(first, vec![first]), (later, vec![first, later])] {
        population
            .observe(
                &mut cx,
                PopulationObservation::Lifecycle {
                    members,
                    changes: vec![Lifecycle::Spawn {
                        handle,
                        origin: Origin::Immigrant,
                    }],
                },
            )
            .unwrap();
        let immigrant = population.get(handle).unwrap();
        assert_eq!(immigrant.mind.genome, RnnGenome::random(&mut reference_cx));
        assert_eq!(cx, reference_cx);
        assert!(
            immigrant
                .mind
                .genome
                .weights
                .iter()
                .all(|weight| (-0.5..0.5).contains(weight))
        );
    }
    assert_ne!(
        population.get(first).unwrap().mind.genome,
        population.get(later).unwrap().mind.genome
    );
}

#[test]
fn observation_is_serialized_and_consumed_once() {
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut genome = RnnGenome::default();
    genome.weights[0] = 0.5;
    genome.weights[32] = 1.0;
    let mut actor = Herbivore::new(genome);
    assert!(actor.act(&mut cx).is_err());
    actor
        .observe(
            &mut cx,
            FoodObservation {
                gradient: DVec2::X,
                strength: 0.0,
            },
        )
        .unwrap();
    assert!(actor.observe(&mut cx, FoodObservation::default()).is_err());
    let mut bytes = Vec::new();
    ciborium::into_writer(&actor, &mut bytes).unwrap();
    let mut restored: Herbivore = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(restored.act(&mut cx).unwrap(), actor.act(&mut cx).unwrap());
    assert_eq!(restored, actor);
    assert!(restored.act(&mut cx).is_err());
}

#[test]
fn invalid_parent_returns_error_without_consuming_randomness() {
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let before = cx.clone();
    let mut population = Herbivores::default();
    let mut allocator = HandleAllocator::new();
    let missing = allocator.allocate().unwrap();
    let child = allocator.allocate().unwrap();
    assert!(
        population
            .observe(
                &mut cx,
                PopulationObservation::Lifecycle {
                    members: vec![child],
                    changes: vec![Lifecycle::Spawn {
                        handle: child,
                        origin: Origin::Child { parent: missing }
                    }],
                }
            )
            .is_err()
    );
    assert_eq!(population.members().count(), 0);
    assert_eq!(cx, before);
}
