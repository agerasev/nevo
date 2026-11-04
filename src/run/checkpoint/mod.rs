//! Import the original checkpoint layout. New checkpoints serialize Universe directly.

use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::Deserialize;

use crate::{
    actor::{
        Handle, HandleAllocator,
        herbivore::{Herbivore, HerbivoreFactory, Herbivores},
    },
    context::Cpu,
    environment::ecosystem::{Animal, Ecosystem, Plant},
    mind::rnn::{Rnn, RnnGenome},
    universe::{ForagingPolicy, Universe},
};

// Stable model identifiers survive source-module and checkpoint-layout changes.
pub const WORLD_MODEL: &str = "foraging-sense-first-v1";
pub const AGENT_MODEL: &str = "herbivore-rnn-3-4-3-v1";

pub fn validate_models(world_model: &str, agent_model: &str) -> Result<()> {
    ensure!(
        world_model == WORLD_MODEL,
        "unsupported world model {world_model}"
    );
    ensure!(
        agent_model == AGENT_MODEL,
        "unsupported agent model {agent_model}"
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacySnapshot {
    format_version: u32,
    world_model: String,
    agent_model: String,
    world: LegacyWorld,
    population: LegacyPopulation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyWorld {
    number: u64,
    rng: Xoshiro256PlusPlus,
    next_animal: u64,
    next_plant: u64,
    animals: BTreeMap<Handle, Animal>,
    plants: Vec<Plant>,
    plant_target: usize,
    immigrant_floor: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPopulation {
    live: BTreeMap<Handle, Rnn>,
    cx: Cpu<Xoshiro256PlusPlus>,
    founder_genome: Option<RnnGenome>,
}

impl LegacySnapshot {
    /// Preserve IDs, recurrent memory and both RNG positions without initialization.
    pub fn into_universe(self) -> Result<Universe> {
        ensure!(
            self.format_version == 1,
            "unsupported snapshot format version {}",
            self.format_version
        );
        validate_models(&self.world_model, &self.agent_model)?;
        let world = self.world;
        // The old template affected founders only. Their genomes already exist;
        // preserving it as an immigration template would change future evolution.
        if let Some(genome) = &self.population.founder_genome {
            ensure!(
                genome.weights.iter().all(|value| value.is_finite()),
                "non-finite founder weight"
            );
        }
        let actor = Herbivores::from_parts(
            self.population
                .live
                .into_iter()
                .map(|(handle, mind)| (handle, Herbivore::from_mind(mind)))
                .collect(),
            HerbivoreFactory::default(),
        );
        let environment = Ecosystem::from_parts(
            world.rng,
            world.next_plant,
            world.animals,
            world.plants,
            world.plant_target,
        );
        let universe = Universe::from_parts(
            world.number,
            environment,
            actor,
            self.population.cx,
            ForagingPolicy::new(world.immigrant_floor),
            HandleAllocator::from_next(world.next_animal)?,
        )?;
        universe.validate_checkpoint()?;
        Ok(universe)
    }
}

#[cfg(test)]
mod tests;
