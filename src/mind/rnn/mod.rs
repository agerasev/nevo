//! Scalar 3–4–3 recurrent network with a fixed topology and evolved parameters.

mod genome;
mod weights;

pub use genome::{RnnGenome, WeightMutation};

use std::array;

use anyhow::Result;

use super::Mind;
use crate::context::Context;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rnn {
    pub genome: RnnGenome,
    pub memory: [f32; 4],
}

impl Rnn {
    pub fn new(genome: RnnGenome) -> Self {
        Self {
            genome,
            memory: [0.0; 4],
        }
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

    fn process(&mut self, _cx: &mut C, input: Self::Input) -> Result<Self::Output> {
        let weights = &self.genome.weights;
        let sensed: [f32; 4] = matrix_vector(&weights[..12], &input);
        // All four recurrent units read the previous tick's memory. The C++ dot
        // product aliased its input and output, overwriting components mid-update.
        let recalled: [f32; 4] = matrix_vector(&weights[12..28], &self.memory);
        self.memory = array::from_fn(|i| (recalled[i] + sensed[i] + weights[28 + i]).tanh());
        let mut output: [f32; 3] = matrix_vector(&weights[32..44], &self.memory);
        for (value, bias) in output.iter_mut().zip(&weights[44..]) {
            *value += bias;
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests;
