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

// Reproduction can use the genome contract without knowing its representation.
fn mutated_copy<C: Context + ?Sized, G: Mutate<C, WeightMutation>>(
    parent: &G,
    param: &WeightMutation,
    cx: &mut C,
) -> Result<G> {
    let mut child = parent.clone();
    child.mutate(param, cx)?;
    Ok(child)
}

#[test]
fn mind_and_genome_accept_an_alternate_unsized_context() {
    let mut context = ResearchContext {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let before = context.clone();
    let mut parent = RnnGenome::default();
    parent.weights[28] = 0.5;
    parent.weights[32] = 1.0;
    parent.weights[46] = 1.0;
    let original = parent.clone();
    let mut agent = AnimalMind::new(parent.clone()).unwrap();

    let cx: &mut dyn Context<Rng = dyn Rng + Send> = &mut context;
    let action = agent.process(FoodObservation::default(), cx).unwrap();
    assert_eq!(action.direction, DVec2::X * f64::from(0.5_f32.tanh()));
    assert_eq!(action.speed_fraction, 1.0_f64.tanh());
    assert_ne!(agent.brain.memory, [0.0; 4]);
    assert_eq!(context, before, "processing must not consume genetic draws");

    let cx: &mut dyn Context<Rng = dyn Rng + Send> = &mut context;
    let param = WeightMutation { max_delta: 0.07 };
    let child = mutated_copy(&parent, &param, cx).unwrap();
    assert_eq!(parent, original);
    assert_ne!(child, parent);
    assert_ne!(context, before);
    for (inherited, original) in child.weights.iter().zip(parent.weights) {
        assert!((inherited - original).abs() <= param.max_delta + f32::EPSILON);
    }
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
fn random_initialization_accepts_unsized_context_without_requiring_a_genome() {
    // No Clone or Genome implementation: random initialization is independent.
    struct RandomLabel(u64);

    impl<C: Context + ?Sized> RandomInit<C> for RandomLabel {
        type Config = ();

        fn random(_: &(), cx: &mut C) -> Result<Self> {
            Ok(Self(cx.rng().next_u64()))
        }
    }

    let mut context = ResearchContext {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let mut expected = context.clone();
    let cx: &mut dyn Context<Rng = dyn Rng + Send> = &mut context;
    let label = RandomLabel::random(&(), cx).unwrap();
    assert_eq!(label.0, expected.rng.next_u64());
    assert_eq!(context, expected);

    let cx: &mut dyn Context<Rng = dyn Rng + Send> = &mut context;
    let genome = RnnGenome::random(&nevo::mind::rnn::Initialization::default(), cx).unwrap();
    assert!(
        genome
            .weights
            .iter()
            .all(|weight| (-0.5..0.5).contains(weight))
    );
    assert_ne!(context, expected);
}
