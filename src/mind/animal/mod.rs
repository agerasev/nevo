//! Map food perception and movement to a replaceable brain's scalar signals.

use anyhow::Result;
use glam::DVec2;

use super::{
    Mind,
    rnn::{Rnn, RnnGenome},
};
use crate::{
    context::Context,
    ecosystem::{FoodObservation, MoveAction},
    metrics::Metrics,
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
    pub fn new(genome: RnnGenome) -> Self {
        Self::from_brain(Rnn::new(genome))
    }
}

impl<M> AnimalMind<M> {
    pub fn from_brain(brain: M) -> Self {
        Self { brain }
    }
}

impl From<RnnGenome> for AnimalMind {
    fn from(genome: RnnGenome) -> Self {
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
    M: Mind<C, Input = [f32; 3], Output = [f32; 3]>,
{
    type Input = FoodObservation;
    type Output = MoveAction;
    type Update = M::Update;
    type Config = Config<M::Config>;

    fn compute_update(
        &self,
        observation: FoodObservation,
        config: &Self::Config,
        cx: &mut C,
    ) -> Result<(MoveAction, Self::Update)> {
        let (output, update) = self.brain.compute_update(
            [
                observation.gradient.x as f32,
                observation.gradient.y as f32,
                observation.strength as f32,
            ],
            &config.brain,
            cx,
        )?;
        Ok((
            MoveAction {
                direction: DVec2::new(output[0].into(), output[1].into()),
                speed_fraction: f64::from(output[2]).tanh().abs(),
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

impl<M: Metrics> Metrics for AnimalMind<M> {
    type Summary = M::Summary;

    fn collect_metrics(&self, summary: &mut Self::Summary) {
        self.brain.collect_metrics(summary);
    }
}

#[cfg(test)]
mod tests;
