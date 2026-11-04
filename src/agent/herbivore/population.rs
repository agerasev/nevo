use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

use super::Herbivore;
use crate::{
    agent::{Agent, AgentId, Population},
    context::Cpu,
    environment::ecosystem::{FoodObservation, MoveAction, Origin},
    genome::Mutate,
    mind::rnn::{RnnGenome, WeightMutation},
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Herbivores {
    live: BTreeMap<AgentId, Herbivore>,
    cx: Cpu<Xoshiro256PlusPlus>,
    founder_genome: Option<RnnGenome>,
}

impl Herbivores {
    pub fn new(genetic_seed: u64) -> Self {
        Self {
            live: BTreeMap::new(),
            cx: Cpu {
                rng: Xoshiro256PlusPlus::seed_from_u64(genetic_seed),
            },
            founder_genome: None,
        }
    }

    /// Use the same known genome for every founder. Immigrants still draw random genomes.
    pub fn with_founder_genome(genetic_seed: u64, genome: RnnGenome) -> Self {
        Self {
            founder_genome: Some(genome),
            ..Self::new(genetic_seed)
        }
    }

    pub fn get(&self, id: AgentId) -> Option<&Herbivore> {
        self.live.get(&id)
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = AgentId> + '_ {
        self.live.keys().copied()
    }

    pub(crate) fn validate_checkpoint(&self) -> Result<()> {
        ensure!(
            self.cx.rng.state() != [0; 32],
            "population RNG state is all zero"
        );
        for agent in self.live.values() {
            ensure!(
                agent.mind.genome.weights.iter().all(|v| v.is_finite()),
                "non-finite genome weight"
            );
            ensure!(
                agent.mind.memory.iter().all(|v| v.is_finite()),
                "non-finite recurrent memory"
            );
        }
        if let Some(genome) = &self.founder_genome {
            ensure!(
                genome.weights.iter().all(|v| v.is_finite()),
                "non-finite founder weight"
            );
        }
        Ok(())
    }
}

impl Population<FoodObservation, MoveAction, Origin> for Herbivores {
    fn spawn(&mut self, id: AgentId, origin: Origin) {
        assert!(!self.live.contains_key(&id), "agent already alive: {id:?}");
        let genome = match origin {
            Origin::Founder => self
                .founder_genome
                .clone()
                .unwrap_or_else(|| RnnGenome::random(&mut self.cx)),
            Origin::Immigrant => RnnGenome::random(&mut self.cx),
            Origin::Child { parent } => {
                let mut genome = self
                    .live
                    .get(&parent)
                    .expect("child requires a living parent")
                    .mind
                    .genome
                    .clone();
                genome
                    .mutate(&WeightMutation::default(), &mut self.cx)
                    .expect("the baseline mutation range is valid");
                genome
            }
        };
        self.live.insert(id, Herbivore::new(genome));
    }

    fn act(&mut self, id: AgentId, obs: &FoodObservation) -> MoveAction {
        self.live
            .get_mut(&id)
            .expect("only a living agent can act")
            .process(&mut self.cx, *obs)
            .expect("the scalar herbivore agent is infallible")
    }

    fn despawn(&mut self, id: AgentId) {
        self.live
            .remove(&id)
            .expect("only a living agent can be removed");
    }
}
