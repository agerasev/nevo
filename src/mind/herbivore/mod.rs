//! Map food perception and movement to a replaceable brain's scalar signals.

mod heredity;

pub use heredity::HerbivoreHeredity;

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

impl<C, M> Mind<C> for HerbivoreMind<M>
where
    C: Context + ?Sized,
    M: Mind<C, Input = [f32; 3], Output = [f32; 3]>,
{
    type Input = FoodObservation;
    type Output = MoveAction;

    fn process(&mut self, cx: &mut C, observation: FoodObservation) -> Result<MoveAction> {
        let output = self.brain.process(
            cx,
            [
                observation.gradient.x as f32,
                observation.gradient.y as f32,
                observation.strength as f32,
            ],
        )?;
        Ok(MoveAction {
            direction: DVec2::new(output[0].into(), output[1].into()),
            speed_fraction: f64::from(output[2]).tanh().abs(),
        })
    }

    fn validate(&self) -> Result<()> {
        self.brain.validate()
    }
}

#[cfg(test)]
mod tests;
