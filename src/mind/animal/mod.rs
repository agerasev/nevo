//! Map body-relative perception and motor control to a replaceable brain.

use anyhow::{Result, ensure};

use super::{
    Mind,
    rnn::{INPUTS, OUTPUTS, Rnn, RnnGenome},
};
use crate::{
    context::Context,
    ecosystem::{AnimalObservation, MotorAction},
};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config<B = super::rnn::Config> {
    pub brain: B,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimalMind<M = Rnn> {
    pub brain: M,
}

impl AnimalMind {
    pub fn new(genome: RnnGenome) -> Result<Self> {
        Ok(Self::from_brain(Rnn::new(genome)?))
    }
}

impl<M> AnimalMind<M> {
    pub fn from_brain(brain: M) -> Self {
        Self { brain }
    }
}

impl TryFrom<RnnGenome> for AnimalMind {
    type Error = anyhow::Error;

    fn try_from(genome: RnnGenome) -> Result<Self> {
        Self::new(genome)
    }
}

impl<M, G: ?Sized> AsRef<G> for AnimalMind<M>
where
    M: AsRef<G>,
{
    fn as_ref(&self) -> &G {
        self.brain.as_ref()
    }
}

impl<C, M> Mind<C> for AnimalMind<M>
where
    C: Context + ?Sized,
    M: Mind<C, Input = [f32; INPUTS], Output = [f32; OUTPUTS]>,
{
    type Input = AnimalObservation;
    type Output = MotorAction;
    type Update = M::Update;
    type Config = Config<M::Config>;

    fn configure(&mut self, config: &Self::Config) -> Result<()> {
        self.brain.configure(&config.brain)
    }

    fn compute_update(
        &self,
        observation: AnimalObservation,
        cx: &mut C,
    ) -> Result<(MotorAction, Self::Update)> {
        let input = [
            observation.gradient.x as f32,
            observation.gradient.y as f32,
            observation.strength as f32,
            observation.velocity.x as f32,
            observation.velocity.y as f32,
            observation.angular_velocity as f32,
        ];
        ensure!(
            input.iter().all(|value| value.is_finite()),
            "non-finite animal signal"
        );
        let (output, update) = self.brain.compute_update(input, cx)?;
        ensure!(
            output.iter().all(|value| value.is_finite()),
            "non-finite brain motor output"
        );
        Ok((
            MotorAction {
                thrust: f64::from(output[0]).tanh().max(0.0),
                turn: f64::from(output[1]).tanh(),
            },
            update,
        ))
    }

    fn apply_update(&mut self, update: Self::Update) {
        self.brain.apply_update(update);
    }

    fn validate(&self) -> Result<()> {
        self.brain.validate()
    }
}

#[cfg(test)]
mod tests;
