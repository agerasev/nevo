use anyhow::Result;
use glam::Vec2;
use nevo::{
    context::{Context, Cpu},
    ecosystem::AnimalObservation,
    genome::{Mutate, RandomInit},
    mind::{
        Mind,
        animal::AnimalMind,
        rnn::{INPUTS, Mutation, OUTPUTS, RnnGenome},
    },
};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn animal_accepts_another_mind_and_propagates_its_errors() {
    struct EchoMind;

    impl<C: Context + ?Sized> nevo::mind::Mind<C> for EchoMind {
        type Input = [f32; INPUTS];
        type Output = [f32; OUTPUTS];
        type Update = ();
        type Config = ();

        fn compute_update(&self, input: Self::Input, _: &mut C) -> Result<(Self::Output, ())> {
            anyhow::ensure!(input[2] >= 0.0, "negative food strength");
            Ok(([input[0], input[1]], ()))
        }

        fn apply_update(&mut self, _: ()) {}
    }

    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut agent = AnimalMind::from_brain(EchoMind);
    let action = agent
        .process(
            AnimalObservation {
                gradient: Vec2::new(0.25, -0.5),
                strength: 1.0,
                ..AnimalObservation::default()
            },
            &mut cx,
        )
        .unwrap();
    assert_eq!(action.thrust, 0.25_f32.tanh());
    assert_eq!(action.turn, (-0.5_f32).tanh());
    assert_eq!(
        agent
            .process(
                AnimalObservation {
                    strength: -1.0,
                    ..AnimalObservation::default()
                },
                &mut cx,
            )
            .unwrap_err()
            .to_string(),
        "negative food strength"
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
    genome.mutate(&Mutation::default(), cx).unwrap();
    AnimalMind::new(genome)
        .unwrap()
        .process(AnimalObservation::default(), cx)
        .unwrap();
}

#[test]
fn disabled_mutation_preserves_genome_and_random_stream() {
    let mut context = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let before = context.clone();
    let mut genome = RnnGenome::default();
    genome.weights.fill(0.5);
    let original = genome.clone();

    genome
        .mutate(
            &Mutation {
                enabled: false,
                ..Default::default()
            },
            &mut context,
        )
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
    let mut genome = RnnGenome::default();
    genome.weights.fill(0.5);
    let original = genome.clone();

    for log_sigma_std in [-1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            genome
                .mutate(
                    &Mutation {
                        log_sigma_std,
                        ..Default::default()
                    },
                    &mut context
                )
                .is_err()
        );
        assert_eq!(genome, original);
        assert_eq!(context, before);
    }
}

#[test]
fn zero_log_variation_preserves_scale_while_weights_mutate() {
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let mut genome = RnnGenome {
        mutation_sigma: 0.07,
        ..Default::default()
    };
    genome
        .mutate(
            &Mutation {
                log_sigma_std: 0.0,
                size_probability: 0.0,
                ..Default::default()
            },
            &mut cx,
        )
        .unwrap();
    assert_eq!(genome.mutation_sigma, 0.07);
    assert!(genome.weights.iter().all(|&w| w.is_finite() && w != 0.0));
}

#[test]
fn log_normal_scales_drive_gaussian_weight_and_bias_noise() {
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(81),
    };
    let parent = RnnGenome {
        mutation_sigma: 0.07,
        ..Default::default()
    };
    let mutation = Mutation {
        log_sigma_std: 0.3,
        size_probability: 0.0,
        ..Default::default()
    };
    let samples = 5000;
    let (mut log_sum, mut log_squares) = (0.0_f64, 0.0_f64);
    let (mut sum, mut squares, mut tails) = (0.0_f64, 0.0_f64, 0);
    for _ in 0..samples {
        let mut child = parent.clone();
        child.mutate(&mutation, &mut cx).unwrap();
        let log_change = f64::from((child.mutation_sigma / parent.mutation_sigma).ln());
        log_sum += log_change;
        log_squares += log_change * log_change;
        for weight in child.weights {
            // This also verifies that weights use the child's new scale.
            let z = f64::from(weight / child.mutation_sigma);
            sum += z;
            squares += z * z;
            tails += usize::from(z.abs() > 3.0);
        }
    }
    let count = (samples * parent.weights.len()) as f64;
    assert!((log_sum / samples as f64).abs() < 0.015);
    assert!((log_squares / samples as f64 - 0.09).abs() < 0.009);
    assert!((sum / count).abs() < 0.015);
    assert!((squares / count - 1.0).abs() < 0.03);
    assert!(tails > 0); // Gaussian noise is not clipped to a uniform interval.
}
