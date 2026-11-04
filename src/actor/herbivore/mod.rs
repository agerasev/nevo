//! Map the ecosystem's food observation and movement action to a mind's signals.

mod factory;

pub use factory::HerbivoreFactory;

use anyhow::{Result, ensure};
use glam::DVec2;

use super::{Actor, population::Population};
use crate::{
    context::Context,
    environment::ecosystem::{FoodObservation, MoveAction},
    mind::{
        Mind,
        rnn::{Rnn, RnnGenome},
    },
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Herbivore<M = Rnn> {
    pub mind: M,
    #[serde(default)]
    pending: Option<FoodObservation>,
}

pub type Herbivores = Population<Herbivore, HerbivoreFactory>;

impl Herbivores {
    /// Use one known genome for every immigrant, including the initial population.
    pub fn with_immigrant_genome(genome: RnnGenome) -> Self {
        Self::new(HerbivoreFactory::with_immigrant_genome(genome))
    }
}

impl Herbivore {
    pub fn new(genome: RnnGenome) -> Self {
        Self::from_mind(Rnn::new(genome))
    }
}

impl<M> Herbivore<M> {
    pub fn from_mind(mind: M) -> Self {
        Self {
            mind,
            pending: None,
        }
    }
}

impl<C, M> Actor<C> for Herbivore<M>
where
    C: Context + ?Sized,
    M: Mind<C, Input = [f32; 3], Output = [f32; 3]>,
{
    type Observation = FoodObservation;
    type Action = MoveAction;

    fn observe(&mut self, _cx: &mut C, observation: FoodObservation) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            "herbivore still has a pending input"
        );
        self.pending = Some(observation);
        Ok(())
    }

    fn act(&mut self, cx: &mut C) -> Result<MoveAction> {
        let obs = self
            .pending
            .take()
            .ok_or_else(|| anyhow::anyhow!("herbivore has no pending input"))?;
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

    fn validate(&self) -> Result<()> {
        if let Some(obs) = self.pending {
            ensure!(
                obs.strength.is_finite() && obs.gradient.is_finite(),
                "non-finite pending observation"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
