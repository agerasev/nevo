use std::array;

use anyhow::{Result, ensure};
use rand::RngExt;

use crate::{
    context::Context,
    genome::{Genome, Mutate},
};

use super::weights;

/// The July 26 controller's row-major input, recurrent and output parameters:
/// 4×3 input weights, 4×4 recurrent weights, 4 biases, 3×4 output weights, 3 biases.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RnnGenome {
    #[serde(with = "weights")]
    pub weights: [f32; 47],
}

impl Default for RnnGenome {
    fn default() -> Self {
        Self { weights: [0.0; 47] }
    }
}

impl RnnGenome {
    pub fn random<C: Context + ?Sized>(cx: &mut C) -> Self {
        Self {
            weights: array::from_fn(|_| cx.rng().random_range(-0.5..0.5)),
        }
    }
}

impl<C: Context + ?Sized> Genome<C> for RnnGenome {}

/// Independent uniform noise in [-max_delta, max_delta) for each weight and bias.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightMutation {
    pub max_delta: f32,
}

impl Default for WeightMutation {
    fn default() -> Self {
        Self { max_delta: 0.02 }
    }
}

impl<C: Context + ?Sized> Mutate<C, WeightMutation> for RnnGenome {
    fn mutate(&mut self, param: &WeightMutation, cx: &mut C) -> Result<()> {
        let delta = param.max_delta;
        ensure!(
            delta >= 0.0 && (2.0 * delta).is_finite(),
            "mutation max_delta must be nonnegative with a finite sampling range"
        );
        if delta > 0.0 {
            for weight in &mut self.weights {
                *weight += cx.rng().random_range(-delta..delta);
            }
        }
        Ok(())
    }
}
