//! Scalar 3–4–3 recurrent network with a fixed topology and evolved parameters.

mod genome;
mod metrics;
mod weights;

pub use genome::{RnnGenome, WeightMutation};
pub use metrics::{GenomeStatistics, GenomeSummary, MetricsConfig, Statistics, Summary};

use std::array;

use anyhow::{Result, ensure};

use super::Mind;
use crate::{context::Context, metrics::Metrics};

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub metrics: MetricsConfig,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rnn {
    pub genome: RnnGenome,
    pub memory: [f32; 4],
    #[serde(skip)]
    metrics_config: Option<MetricsConfig>,
    #[serde(skip)]
    metrics: Summary,
}

// Observation caches do not participate in simulation-state equality.
impl PartialEq for Rnn {
    fn eq(&self, other: &Self) -> bool {
        self.genome == other.genome && self.memory == other.memory
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Update {
    pub memory: [f32; 4],
    metrics: Summary,
}

impl Rnn {
    pub fn new(genome: RnnGenome) -> Self {
        Self {
            genome,
            memory: [0.0; 4],
            metrics_config: None,
            metrics: Summary::default(),
        }
    }
}

impl From<RnnGenome> for Rnn {
    fn from(genome: RnnGenome) -> Self {
        Self::new(genome)
    }
}

impl AsRef<RnnGenome> for Rnn {
    fn as_ref(&self) -> &RnnGenome {
        &self.genome
    }
}

fn matrix_vector<const IN: usize, const OUT: usize>(
    weights: &[f32],
    input: &[f32; IN],
) -> [f32; OUT] {
    debug_assert_eq!(weights.len(), IN * OUT);
    array::from_fn(|i| {
        weights[i * IN..(i + 1) * IN]
            .iter()
            .zip(input)
            .map(|(weight, value)| weight * value)
            .sum()
    })
}

impl<C: Context + ?Sized> Mind<C> for Rnn {
    type Input = [f32; 3];
    type Output = [f32; 3];
    type Update = Update;
    type Config = Config;

    fn configure(&mut self, config: &Config, metrics_enabled: bool) -> Result<()> {
        self.metrics_config = (metrics_enabled
            && (config.metrics.activation.enabled() || config.metrics.saturation))
            .then_some(config.metrics);
        self.metrics = Summary::default();
        Ok(())
    }

    fn compute_update(&self, input: Self::Input, _cx: &mut C) -> Result<(Self::Output, Update)> {
        <Self as Mind<C>>::validate(self)?;
        let weights = &self.genome.weights;
        let sensed: [f32; 4] = matrix_vector(&weights[..12], &input);
        // All four recurrent units read the previous tick's memory. The C++ dot
        // product aliased its input and output, overwriting components mid-update.
        let recalled: [f32; 4] = matrix_vector(&weights[12..28], &self.memory);
        let memory: [f32; 4] =
            array::from_fn(|i| (recalled[i] + sensed[i] + weights[28 + i]).tanh());
        ensure!(
            memory.iter().all(|value| value.is_finite()),
            "non-finite prepared recurrent memory"
        );
        let mut output: [f32; 3] = matrix_vector(&weights[32..44], &memory);
        for (value, bias) in output.iter_mut().zip(&weights[44..]) {
            *value += bias;
        }

        let metrics = self
            .metrics_config
            .as_ref()
            .map(|config| Summary::from_memory(&memory, config))
            .unwrap_or_default();
        Ok((output, Update { memory, metrics }))
    }

    fn apply_update(&mut self, update: Update) {
        self.memory = update.memory;
        self.metrics = update.metrics;
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.genome.weights.iter().all(|value| value.is_finite()),
            "non-finite genome weight"
        );
        ensure!(
            self.memory.iter().all(|value| value.is_finite()),
            "non-finite recurrent memory"
        );
        Ok(())
    }
}

impl Metrics for Rnn {
    type Summary = Summary;

    fn collect_metrics(&self, summary: &mut Summary) {
        summary.merge(&self.metrics);
    }
}

#[cfg(test)]
mod tests;
