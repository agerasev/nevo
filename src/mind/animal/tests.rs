use super::*;
use crate::context::Cpu;
use glam::Vec2;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[derive(Default)]
struct Probe {
    output: [f32; OUTPUTS],
    input: Option<[f32; INPUTS]>,
}

impl<C: Context + ?Sized> Mind<C> for Probe {
    type Input = [f32; INPUTS];
    type Output = [f32; OUTPUTS];
    type Update = Self::Input;
    type Config = ();

    fn compute_update(
        &self,
        input: Self::Input,
        _: &mut C,
    ) -> Result<(Self::Output, Self::Update)> {
        Ok((self.output, input))
    }

    fn apply_update(&mut self, input: Self::Update) {
        self.input = Some(input);
    }
}

fn context() -> Cpu<Xoshiro256PlusPlus> {
    Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(7),
    }
}

#[test]
fn observation_signals_keep_body_order_and_zero_outputs_are_idle() {
    let mut mind = AnimalMind::from_brain(Probe::default());
    let action = mind
        .process(
            AnimalObservation {
                gradient: Vec2::new(0.25, -0.5),
                strength: 2.0,
                velocity: Vec2::new(1.5, -2.5),
                angular_velocity: 0.75,
            },
            &mut context(),
        )
        .unwrap();
    assert_eq!(mind.brain.input, Some([0.25, -0.5, 2.0, 1.5, -2.5, 0.75]));
    assert_eq!(action.thrust, 0.0);
    assert_eq!(action.turn, 0.0);
}

#[test]
fn motor_outputs_allow_only_forward_thrust_and_signed_turning() {
    let mut mind = AnimalMind::from_brain(Probe::default());
    for (output, thrust, turn) in [
        ([2.0, -1.0], 2.0_f32.tanh(), (-1.0_f32).tanh()),
        ([-2.0, 0.0], 0.0, 0.0),
        ([f32::MAX, -f32::MAX], 1.0, -1.0),
    ] {
        mind.brain.output = output;
        let action = mind
            .process(AnimalObservation::default(), &mut context())
            .unwrap();
        assert_eq!(action.thrust, thrust);
        assert_eq!(action.turn, turn);
    }
}

#[test]
fn invalid_signals_and_outputs_do_not_apply_brain_updates() {
    let mut mind = AnimalMind::from_brain(Probe::default());
    for observation in [
        AnimalObservation {
            strength: f32::NAN,
            ..AnimalObservation::default()
        },
        AnimalObservation {
            velocity: Vec2::splat(f32::INFINITY),
            ..AnimalObservation::default()
        },
    ] {
        assert!(mind.process(observation, &mut context()).is_err());
        assert!(mind.brain.input.is_none());
    }
    for output in [[f32::NAN, 0.0], [0.0, f32::INFINITY]] {
        mind.brain.output = output;
        assert!(
            mind.process(AnimalObservation::default(), &mut context())
                .is_err()
        );
        assert!(mind.brain.input.is_none());
    }
}

#[test]
fn constructing_from_a_genome_resets_memory() {
    let mut parent = AnimalMind::new(RnnGenome::default()).unwrap();
    parent.brain.memory.fill(0.5);
    let child = AnimalMind::try_from(parent.as_ref().clone()).unwrap();
    assert_eq!(child.as_ref(), parent.as_ref());
    assert_eq!(child.brain.memory, [0.0; 4]);
    assert_eq!(parent.brain.memory, [0.5; 4]);
}
