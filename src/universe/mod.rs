//! Complete simulation state, independent of the experiment's internal ownership.

use anyhow::{Result, ensure};

use crate::{
    ecosystem::{Ecosystem, Scenario},
    run::Run,
};

/// An experiment owns its physical state, minds, random streams, and rules.
/// `advance` operates on private prepared state; it may fail without publication.
pub trait Simulation {
    type Report;

    fn advance(&mut self, step: u64) -> Result<Self::Report>;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Everything needed to continue a simulation. Locks and viewers live outside it.
/// Clone must produce independently mutable state. Reconstructible caches may be
/// skipped by serde; RNG positions and recurrent memory are required state.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Universe<S = Ecosystem> {
    pub(crate) step: u64,
    pub(crate) world: S,
}

impl<S> Universe<S> {
    pub fn step(&self) -> u64 {
        self.step
    }

    pub fn world(&self) -> &S {
        &self.world
    }
}

impl<S: Simulation> Universe<S> {
    pub fn from_world(world: S) -> Result<Self> {
        Self::from_parts(0, world)
    }

    pub fn from_parts(step: u64, world: S) -> Result<Self> {
        let universe = Self { step, world };
        universe.validate()?;
        Ok(universe)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(self.step < u64::MAX, "universe step counter is exhausted");
        self.world.validate()
    }

    pub fn restore(self) -> Result<Run<S>> {
        self.validate()?;
        Ok(Run::from_universe(self))
    }
}

impl<S: Simulation + Clone> Universe<S> {
    /// Compute a complete next state without changing the published universe.
    /// Component deltas can replace this full clone in a subsequent change.
    pub fn prepare_step(&self) -> Result<(Self, S::Report)> {
        let step = self
            .step
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("universe step counter is exhausted"))?;
        let mut next = self.clone();
        let report = next.world.advance(step)?;
        next.step = step;
        next.validate()?;
        Ok((next, report))
    }

    pub fn advance(&mut self) -> Result<S::Report> {
        let (next, report) = self.prepare_step()?;
        *self = next;
        Ok(report)
    }
}

impl Universe {
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self> {
        Self::from_world(Ecosystem::new(scenario, seed)?)
    }
}

#[cfg(test)]
mod tests;
