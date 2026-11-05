//! Scalar recurrent network with configurable hidden width and evolved parameters.

mod genome;
mod metrics;

pub use genome::{Initialization, RnnGenome, WeightMutation};
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
    pub memory: Vec<f32>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub memory: Vec<f32>,
    metrics: Summary,
}

impl Rnn {
    pub fn new(genome: RnnGenome) -> Result<Self> {
        genome.validate()?;
        let mut memory = Vec::new();
        memory.try_reserve_exact(genome.hidden_units)?;
        memory.resize(genome.hidden_units, 0.0);
        Ok(Self {
            genome,
            memory,
            metrics_config: None,
            metrics: Summary::default(),
        })
    }
}

impl TryFrom<RnnGenome> for Rnn {
    type Error = anyhow::Error;

    fn try_from(genome: RnnGenome) -> Result<Self> {
        Self::new(genome)
    }
}

impl AsRef<RnnGenome> for Rnn {
    fn as_ref(&self) -> &RnnGenome {
        &self.genome
    }
}

fn dot(weights: &[f32], input: &[f32]) -> f32 {
    debug_assert_eq!(weights.len(), input.len());
    weights
        .iter()
        .zip(input)
        .map(|(weight, value)| weight * value)
        .sum()
}

impl<C: Context + ?Sized> Mind<C> for Rnn {
    type Input = [f32; 3];
    type Output = [f32; 3];
    type Update = Update;
    type Config = Config;

    fn configure(&mut self, config: &Config, metrics_enabled: bool) -> Result<()> {
        if metrics_enabled {
            config.metrics.validate()?;
        }
        self.metrics_config = (metrics_enabled
            && (config.metrics.activation.enabled() || config.metrics.saturation))
            .then_some(config.metrics);
        self.metrics = Summary::default();
        Ok(())
    }

    fn compute_update(&self, input: Self::Input, _cx: &mut C) -> Result<(Self::Output, Update)> {
        <Self as Mind<C>>::validate(self)?;
        let weights = &self.genome.weights;
        let hidden = self.genome.hidden_units;
        let recurrent_offset = 3 * hidden;
        let hidden_bias_offset = recurrent_offset + hidden * hidden;
        let output_offset = hidden_bias_offset + hidden;
        let output_bias_offset = output_offset + 3 * hidden;
        // Every recurrent unit reads the previous tick's memory. The C++ dot
        // product aliased its input and output, overwriting components mid-update.
        let memory: Vec<_> = (0..hidden)
            .map(|i| {
                let sensed = dot(&weights[3 * i..3 * (i + 1)], &input);
                let begin = recurrent_offset + hidden * i;
                let recalled = dot(&weights[begin..begin + hidden], &self.memory);
                (recalled + sensed + weights[hidden_bias_offset + i]).tanh()
            })
            .collect();
        ensure!(
            memory.iter().all(|value| value.is_finite()),
            "non-finite prepared recurrent memory"
        );
        let mut output: [f32; 3] = array::from_fn(|i| {
            let begin = output_offset + hidden * i;
            dot(&weights[begin..begin + hidden], &memory)
        });
        for (value, bias) in output.iter_mut().zip(&weights[output_bias_offset..]) {
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
        self.genome.validate()?;
        ensure!(
            self.memory.len() == self.genome.hidden_units,
            "recurrent memory length differs from hidden_units"
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
