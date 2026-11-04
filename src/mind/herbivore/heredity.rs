use anyhow::{Result, ensure};

use super::HerbivoreMind;
use crate::{
    context::Context,
    genome::Mutate,
    mind::{
        Heredity,
        rnn::{RnnGenome, WeightMutation},
    },
};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerbivoreHeredity {
    immigrant_genome: Option<RnnGenome>,
}

impl HerbivoreHeredity {
    /// Apply a known genome to every immigrant, including initial arrivals.
    pub fn with_immigrant_genome(genome: RnnGenome) -> Self {
        Self {
            immigrant_genome: Some(genome),
        }
    }
}

impl<C: Context + ?Sized> Heredity<C, HerbivoreMind> for HerbivoreHeredity {
    fn immigrant(&self, cx: &mut C) -> Result<HerbivoreMind> {
        let genome = self
            .immigrant_genome
            .clone()
            .unwrap_or_else(|| RnnGenome::random(cx));
        Ok(HerbivoreMind::new(genome))
    }

    fn offspring(&self, cx: &mut C, parent: &HerbivoreMind) -> Result<HerbivoreMind> {
        let mut genome = parent.brain.genome.clone();
        genome.mutate(&WeightMutation::default(), cx)?;
        Ok(HerbivoreMind::new(genome))
    }

    fn validate(&self) -> Result<()> {
        if let Some(genome) = &self.immigrant_genome {
            ensure!(
                genome.weights.iter().all(|value| value.is_finite()),
                "non-finite immigrant weight"
            );
        }
        Ok(())
    }
}
