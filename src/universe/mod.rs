//! Complete simulation state, independent of the experiment's internal ownership.

use anyhow::{Result, ensure};

use crate::{
    ecosystem::{Ecosystem, Scenario},
    run::Run,
};

/// An experiment owns its physical state, minds, random streams, and rules.
/// Preparation reads published state and returns an owned, validated update.
/// It must not change simulation state through interior mutability. All fallible
/// work and random draws belong here, using private working state as needed.
/// Applying an update to its unchanged source state must not fail or draw randomness.
pub trait Simulation {
    type Update;
    type Report;

    fn prepare_step(&self, step: u64) -> Result<(Self::Update, Self::Report)>;
    fn apply_step(&mut self, update: Self::Update);

    fn advance(&mut self, step: u64) -> Result<Self::Report> {
        let (update, report) = self.prepare_step(step)?;
        self.apply_step(update);
        Ok(report)
    }

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

/// Confined to the stepping path, which applies it to the unchanged source Universe.
pub(crate) struct PreparedStep<S: Simulation> {
    step: u64,
    world: S::Update,
    report: S::Report,
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

    pub(crate) fn prepare_step(&self) -> Result<PreparedStep<S>> {
        let step = self
            .step
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("universe step counter is exhausted"))?;
        ensure!(step < u64::MAX, "universe step counter is exhausted");
        let (world, report) = self.world.prepare_step(step)?;
        Ok(PreparedStep {
            step,
            world,
            report,
        })
    }

    pub(crate) fn apply_step(&mut self, update: PreparedStep<S>) -> S::Report {
        self.world.apply_step(update.world);
        self.step = update.step;
        update.report
    }

    pub fn advance(&mut self) -> Result<S::Report> {
        let update = self.prepare_step()?;
        Ok(self.apply_step(update))
    }
}

impl Universe {
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self> {
        Self::from_world(Ecosystem::new(scenario, seed)?)
    }
}

#[cfg(test)]
mod tests;
