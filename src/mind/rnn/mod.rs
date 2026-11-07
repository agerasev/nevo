//! Scalar recurrent network with configurable hidden width and evolved parameters.

mod genome;

pub use genome::{Initialization, Mutation, RnnGenome};

use std::array;

use anyhow::{Result, ensure};

use super::Mind;
use crate::context::Context;

pub const INPUTS: usize = 6;
pub const OUTPUTS: usize = 2;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rnn {
    pub genome: RnnGenome,
    pub memory: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub memory: Vec<f32>,
}

impl Rnn {
    pub fn new(genome: RnnGenome) -> Result<Self> {
        genome.validate()?;
        let mut memory = Vec::new();
        memory.try_reserve_exact(genome.hidden_units)?;
        memory.resize(genome.hidden_units, 0.0);
        Ok(Self { genome, memory })
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
    type Input = [f32; INPUTS];
    type Output = [f32; OUTPUTS];
    type Update = Update;
    type Config = Config;

    fn compute_update(&self, input: Self::Input, _cx: &mut C) -> Result<(Self::Output, Update)> {
        ensure!(
            input.iter().all(|value| value.is_finite()),
            "non-finite recurrent input"
        );
        <Self as Mind<C>>::validate(self)?;
        let weights = &self.genome.weights;
        let hidden = self.genome.hidden_units;
        let recurrent_offset = INPUTS * hidden;
        let hidden_bias_offset = recurrent_offset + hidden * hidden;
        let output_offset = hidden_bias_offset + hidden;
        let output_bias_offset = output_offset + OUTPUTS * hidden;
        // Every recurrent unit reads the previous tick's memory. The C++ dot
        // product aliased its input and output, overwriting components mid-update.
        let memory: Vec<_> = (0..hidden)
            .map(|i| {
                let sensed = dot(&weights[INPUTS * i..INPUTS * (i + 1)], &input);
                let begin = recurrent_offset + hidden * i;
                let recalled = dot(&weights[begin..begin + hidden], &self.memory);
                (recalled + sensed + weights[hidden_bias_offset + i]).tanh()
            })
            .collect();
        ensure!(
            memory.iter().all(|value| value.is_finite()),
            "non-finite prepared recurrent memory"
        );
        let mut output: [f32; OUTPUTS] = array::from_fn(|i| {
            let begin = output_offset + hidden * i;
            dot(&weights[begin..begin + hidden], &memory)
        });
        for (value, bias) in output.iter_mut().zip(&weights[output_bias_offset..]) {
            *value += bias;
        }
        ensure!(
            output.iter().all(|value| value.is_finite()),
            "non-finite recurrent output"
        );

        Ok((output, Update { memory }))
    }

    fn apply_update(&mut self, update: Update) {
        self.memory = update.memory;
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

#[cfg(test)]
mod tests;
