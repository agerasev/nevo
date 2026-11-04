use anyhow::Result;
use glam::DVec2;
use nevo::{
    Agent, Context, Cpu, Mutate,
    foraging::FoodObservation,
    herbivore::{Herbivore, HerbivoreGenome, WeightMutation},
};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

#[derive(Clone, Debug, PartialEq)]
struct ResearchContext {
    rng: Xoshiro256PlusPlus,
}

impl Context for ResearchContext {
    type Rng = dyn Rng + Send;
    type DType = &'static str;
    type Device = u8;

    fn rng(&mut self) -> &mut Self::Rng {
        &mut self.rng
    }

    fn dtype(&self) -> Self::DType {
        "f32"
    }

    fn device(&self) -> Self::Device {
        0
    }
}

// A population can use the genome contract without knowing its representation.
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
fn agent_and_genome_accept_an_alternate_unsized_context() {
    let mut context = ResearchContext {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let before = context.clone();
    let mut parent = HerbivoreGenome::default();
    parent.weights[28] = 0.5;
    parent.weights[32] = 1.0;
    parent.weights[46] = 1.0;
    let original = parent.clone();
    let mut agent = Herbivore::new(parent.clone());

    let cx: &mut dyn Context<Rng = dyn Rng + Send, DType = &'static str, Device = u8> =
        &mut context;
    let action = agent.process(cx, FoodObservation::default()).unwrap();
    assert_eq!(action.direction, DVec2::X * f64::from(0.5_f32.tanh()));
    assert_eq!(action.speed_fraction, 1.0_f64.tanh());
    assert_ne!(agent.memory, [0.0; 4]);
    assert_eq!(context, before, "processing must not consume genetic draws");

    let cx: &mut dyn Context<Rng = dyn Rng + Send, DType = &'static str, Device = u8> =
        &mut context;
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
    let mut genome = HerbivoreGenome { weights: [0.5; 47] };
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
    let mut genome = HerbivoreGenome { weights: [0.5; 47] };
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
