//! Map the ecosystem's food observation and movement action to a mind's signals.

mod population;

pub use population::Herbivores;

use anyhow::Result;
use glam::DVec2;

use super::Agent;
use crate::{
    context::Context,
    environment::ecosystem::{FoodObservation, MoveAction},
    mind::{
        Mind,
        rnn::{Rnn, RnnGenome},
    },
};

// Preserve the existing checkpoint representation: genome and memory at this level.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Herbivore<M = Rnn> {
    pub mind: M,
}

impl Herbivore {
    pub fn new(genome: RnnGenome) -> Self {
        Self::from_mind(Rnn::new(genome))
    }
}

impl<M> Herbivore<M> {
    pub fn from_mind(mind: M) -> Self {
        Self { mind }
    }
}

impl<C, M> Agent<C> for Herbivore<M>
where
    C: Context + ?Sized,
    M: Mind<C, Input = [f32; 3], Output = [f32; 3]>,
{
    type Input = FoodObservation;
    type Output = MoveAction;

    fn process(&mut self, cx: &mut C, obs: FoodObservation) -> Result<MoveAction> {
        let output = self.mind.process(
            cx,
            [
                obs.gradient.x as f32,
                obs.gradient.y as f32,
                obs.strength as f32,
            ],
        )?;
        Ok(MoveAction {
            direction: DVec2::new(output[0].into(), output[1].into()),
            speed_fraction: f64::from(output[2]).tanh().abs(),
        })
    }
}

#[cfg(test)]
mod tests;
