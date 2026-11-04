//! Complete ecosystem state at a tick boundary. Serialization is an adapter;
//! the generic agent, environment, and runner contracts stay format-independent.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{agent::herbivore::Herbivores, environment::ecosystem::Ecosystem, run::Run};

const FORMAT_VERSION: u32 = 1;
// Stable on-disk identifier; renaming Rust modules does not change the world model.
const WORLD_MODEL: &str = "foraging-sense-first-v1";
const AGENT_MODEL: &str = "herbivore-rnn-3-4-3-v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    format_version: u32,
    world_model: String,
    agent_model: String,
    world: Ecosystem,
    population: Herbivores,
}

impl Snapshot {
    /// Call from the simulation owner between completed ticks.
    pub fn capture(run: &Run) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            world_model: WORLD_MODEL.into(),
            agent_model: AGENT_MODEL.into(),
            world: run.world().read().clone(),
            population: run.population.clone(),
        }
    }

    pub fn tick(&self) -> u64 {
        self.world.number()
    }

    /// Recreate synchronization only; never initialize agents or draw new seeds.
    pub fn restore(self) -> Result<Run> {
        ensure!(
            self.format_version == FORMAT_VERSION,
            "unsupported snapshot format version {}",
            self.format_version
        );
        ensure!(
            self.world_model == WORLD_MODEL,
            "unsupported world model {}",
            self.world_model
        );
        ensure!(
            self.agent_model == AGENT_MODEL,
            "unsupported agent model {}",
            self.agent_model
        );
        self.world.validate_checkpoint()?;
        self.population.validate_checkpoint()?;
        ensure!(
            self.world
                .animals()
                .keys()
                .copied()
                .eq(self.population.ids()),
            "world body IDs differ from population IDs"
        );
        Ok(Run::from_parts(self.world, self.population))
    }
}

#[cfg(test)]
mod tests;
