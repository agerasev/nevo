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
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerbivoreMind<M = Rnn> {
    pub brain: M,
}

impl HerbivoreMind {
    pub fn new(genome: RnnGenome) -> Self {
        Self::from_brain(Rnn::new(genome))
    }
}

impl<M> HerbivoreMind<M> {
    pub fn from_brain(brain: M) -> Self {
        Self { brain }
    }
}

impl From<RnnGenome> for HerbivoreMind {
    fn from(genome: RnnGenome) -> Self {
        Self::new(genome)
    }
}

impl<M, G: ?Sized> AsRef<G> for HerbivoreMind<M>
where
    M: AsRef<G>,
{
    fn as_ref(&self) -> &G {
        self.brain.as_ref()
    }
}

impl<C, M> Mind<C> for HerbivoreMind<M>
where
    C: Context + ?Sized,
    M: Mind<C, Input = [f32; 3], Output = [f32; 3]>,
{
    type Input = FoodObservation;
    type Output = MoveAction;
    type Update = M::Update;

    fn prepare(
        &self,
        cx: &mut C,
        observation: FoodObservation,
    ) -> Result<(MoveAction, Self::Update)> {
        let (output, update) = self.brain.prepare(
            cx,
            [
                observation.gradient.x as f32,
                observation.gradient.y as f32,
                observation.strength as f32,
            ],
        )?;
        Ok((
            MoveAction {
                direction: DVec2::new(output[0].into(), output[1].into()),
                speed_fraction: f64::from(output[2]).tanh().abs(),
            },
            update,
        ))
    }

    fn apply(&mut self, update: Self::Update) {
        self.brain.apply(update);
    }

    fn validate(&self) -> Result<()> {
        self.brain.validate()
    }
}

#[cfg(test)]
mod tests;
