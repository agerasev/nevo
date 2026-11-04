use std::collections::BTreeMap;

use anyhow::{Result, ensure};

use super::Herbivore;
use crate::{
    actor::{Handle, population::Factory},
    context::Context,
    environment::ecosystem::Origin,
    genome::Mutate,
    mind::rnn::{RnnGenome, WeightMutation},
};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerbivoreFactory {
    immigrant_genome: Option<RnnGenome>,
}

impl HerbivoreFactory {
    pub fn with_immigrant_genome(genome: RnnGenome) -> Self {
        Self {
            immigrant_genome: Some(genome),
        }
    }
}

impl<C: Context + ?Sized> Factory<C, Herbivore> for HerbivoreFactory {
    type Origin = Origin;

    fn create(
        &self,
        cx: &mut C,
        origin: Origin,
        existing: &BTreeMap<Handle, Herbivore>,
    ) -> Result<Herbivore> {
        let genome = match origin {
            Origin::Immigrant => self
                .immigrant_genome
                .clone()
                .unwrap_or_else(|| RnnGenome::random(cx)),
            Origin::Child { parent } => {
                let mut genome = existing
                    .get(&parent)
                    .ok_or_else(|| anyhow::anyhow!("child requires living parent {parent:?}"))?
                    .mind
                    .genome
                    .clone();
                genome.mutate(&WeightMutation::default(), cx)?;
                genome
            }
        };
        Ok(Herbivore::new(genome))
    }

    fn validate(&self, existing: &BTreeMap<Handle, Herbivore>) -> Result<()> {
        for actor in existing.values() {
            ensure!(
                actor.mind.genome.weights.iter().all(|v| v.is_finite()),
                "non-finite genome weight"
            );
            ensure!(
                actor.mind.memory.iter().all(|v| v.is_finite()),
                "non-finite recurrent memory"
            );
        }
        if let Some(genome) = &self.immigrant_genome {
            ensure!(
                genome.weights.iter().all(|v| v.is_finite()),
                "non-finite immigrant weight"
            );
        }
        Ok(())
    }
}
