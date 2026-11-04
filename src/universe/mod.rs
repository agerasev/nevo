//! Complete simulation state, independent of the experiment's internal ownership.

#[cfg(feature = "viewer")]
mod draw;

use anyhow::{Result, ensure};

use crate::{
    ecosystem::{Config, Ecosystem},
    metrics::Metrics,
    run::Run,
};

/// An experiment owns its physical state, minds, random streams, and rules.
/// Computation reads published state and returns an owned, validated update.
/// It must not change simulation state through interior mutability. All fallible
/// work and random draws belong here, using private working state as needed.
/// Applying an update to its unchanged source state must not fail or draw randomness.
pub trait Simulation {
    type Config: Default;
    type Update;
    type Report;

    fn compute_update(
        &self,
        step: u64,
        config: &Self::Config,
    ) -> Result<(Self::Report, Self::Update)>;
    fn apply_update(&mut self, update: Self::Update);

    fn advance(&mut self, step: u64, config: &Self::Config) -> Result<Self::Report> {
        let (report, update) = self.compute_update(step, config)?;
        self.apply_update(update);
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
pub(crate) struct UniverseUpdate<S: Simulation> {
    step: u64,
    world: S::Update,
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

    pub fn restore(self, config: S::Config) -> Result<Run<S>> {
        self.validate()?;
        Ok(Run::from_universe(self, config))
    }

    pub(crate) fn compute_update(
        &self,
        config: &S::Config,
    ) -> Result<(S::Report, UniverseUpdate<S>)> {
        let step = self
            .step
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("universe step counter is exhausted"))?;
        ensure!(step < u64::MAX, "universe step counter is exhausted");
        let (report, world) = self.world.compute_update(step, config)?;
        Ok((report, UniverseUpdate { step, world }))
    }

    pub(crate) fn apply_update(&mut self, update: UniverseUpdate<S>) {
        self.world.apply_update(update.world);
        self.step = update.step;
    }

    pub fn advance(&mut self, config: &S::Config) -> Result<S::Report> {
        let (report, update) = self.compute_update(config)?;
        self.apply_update(update);
        Ok(report)
    }
}

impl<S: Metrics> Metrics for Universe<S> {
    type Summary = S::Summary;

    fn collect_metrics(&self, summary: &mut Self::Summary) {
        self.world.collect_metrics(summary);
    }
}

impl Universe {
    pub fn new(config: Config, seed: u64) -> Result<Self> {
        Self::from_world(Ecosystem::new(config, seed)?)
    }
}

#[cfg(test)]
mod tests;
