use anyhow::Result;
use glam::DVec2;
use nevo::{
    context::{Context, Cpu},
    ecosystem::FoodObservation,
    genome::{Mutate, RandomInit},
    mind::{
        Mind,
        animal::AnimalMind,
        rnn::{RnnGenome, WeightMutation},
    },
};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn animal_accepts_another_mind_and_propagates_its_errors() {
    struct EchoMind;

    impl<C: Context + ?Sized> nevo::mind::Mind<C> for EchoMind {
        type Input = [f32; 3];
        type Output = [f32; 3];
        type Update = ();
        type Config = ();

        fn compute_update(&self, input: Self::Input, _: &mut C) -> Result<(Self::Output, ())> {
            anyhow::ensure!(input.iter().all(|v| v.is_finite()), "invalid signal");
            Ok((input, ()))
        }

        fn apply_update(&mut self, _: ()) {}
    }

    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut agent = AnimalMind::from_brain(EchoMind);
    let action = agent
        .process(
            FoodObservation {
                gradient: DVec2::new(0.25, -0.5),
                strength: -1.0,
            },
            &mut cx,
        )
        .unwrap();
    assert_eq!(action.direction, DVec2::new(0.25, -0.5));
    assert_eq!(action.speed_fraction, 1.0_f64.tanh());
    assert_eq!(
        agent
            .process(
                FoodObservation {
                    strength: f64::NAN,
                    ..FoodObservation::default()
                },
                &mut cx,
            )
            .unwrap_err()
            .to_string(),
        "invalid signal"
    );
}

#[derive(Clone, Debug, PartialEq)]
struct ResearchContext {
    rng: Xoshiro256PlusPlus,
}

impl Context for ResearchContext {
    type Rng = dyn Rng + Send;

    fn rng(&mut self) -> &mut Self::Rng {
        &mut self.rng
    }
}

#[test]
fn mind_initialization_and_mutation_accept_an_unsized_context() {
    let mut context = ResearchContext {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let cx: &mut dyn Context<Rng = dyn Rng + Send> = &mut context;
    let mut genome = RnnGenome::random(&Default::default(), cx).unwrap();
    genome.mutate(&WeightMutation::default(), cx).unwrap();
    AnimalMind::new(genome)
        .unwrap()
        .process(FoodObservation::default(), cx)
        .unwrap();
}

#[test]
fn zero_mutation_preserves_genome_and_random_stream() {
    let mut context = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let before = context.clone();
    let mut genome = RnnGenome {
        weights: vec![0.5; 47],
        ..RnnGenome::default()
    };
    let original = genome.clone();

    genome
        .mutate(&WeightMutation { max_delta: 0.0 }, &mut context)
        .unwrap();
    assert_eq!(genome, original);
    assert_eq!(context, before);
}

#[test]
fn invalid_mutation_fails_without_changing_genome_or_random_stream() {
    let mut context = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let before = context.clone();
    let mut genome = RnnGenome {
        weights: vec![0.5; 47],
        ..RnnGenome::default()
    };
    let original = genome.clone();

    for max_delta in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
        assert!(
            genome
                .mutate(&WeightMutation { max_delta }, &mut context)
                .is_err()
        );
        assert_eq!(genome, original);
        assert_eq!(context, before);
    }
}

#[test]
fn nonzero_mutation_changes_weights_within_the_configured_range() {
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let mut genome = RnnGenome::default();
    genome
        .mutate(&WeightMutation { max_delta: 0.07 }, &mut cx)
        .unwrap();
    assert!(genome.weights.iter().any(|&w| w != 0.0));
    assert!(genome.weights.iter().all(|w| (-0.07..0.07).contains(w)));
}
