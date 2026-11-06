use anyhow::{Result, ensure};
use rand::RngExt;

use super::{INPUTS, OUTPUTS};
use crate::{
    context::Context,
    genome::{Genome, Mutate, RandomInit},
};

/// Initialization for a recurrent brain with fixed input/output dimensions.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Initialization {
    pub hidden_units: usize,
    pub weight_min: f32,
    pub weight_max: f32,
}

impl Default for Initialization {
    fn default() -> Self {
        Self {
            hidden_units: 4,
            weight_min: -0.5,
            weight_max: 0.5,
        }
    }
}

impl Initialization {
    pub fn validate(&self) -> Result<()> {
        weight_count(self.hidden_units)?;
        ensure!(
            self.weight_min.is_finite()
                && self.weight_max.is_finite()
                && self.weight_min <= self.weight_max
                && (self.weight_max - self.weight_min).is_finite(),
            "initial weight range must be finite and nondecreasing"
        );
        Ok(())
    }
}

/// Checked shape arithmetic also rejects lengths that a Rust allocation cannot hold.
pub(super) fn weight_count(hidden_units: usize) -> Result<usize> {
    ensure!(hidden_units > 0, "hidden_units must be positive");
    let count = hidden_units
        .checked_add(INPUTS + OUTPUTS + 1)
        .and_then(|n| n.checked_mul(hidden_units))
        .and_then(|n| n.checked_add(OUTPUTS))
        .filter(|&n| n <= isize::MAX as usize / size_of::<f32>())
        .ok_or_else(|| anyhow::anyhow!("hidden_units produces an oversized genome"))?;
    Ok(count)
}

/// Row-major input weights, recurrent weights, hidden biases, output weights,
/// and output biases. Width is inherited together with the weight coordinates.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RnnGenome {
    pub hidden_units: usize,
    pub weights: Vec<f32>,
}

impl Default for RnnGenome {
    fn default() -> Self {
        let hidden_units = Initialization::default().hidden_units;
        Self {
            hidden_units,
            weights: vec![0.0; weight_count(hidden_units).expect("default shape is valid")],
        }
    }
}

impl RnnGenome {
    pub fn validate(&self) -> Result<()> {
        let expected = weight_count(self.hidden_units)?;
        ensure!(
            self.weights.len() == expected,
            "expected {expected} weights for {} hidden units, got {}",
            self.hidden_units,
            self.weights.len()
        );
        ensure!(
            self.weights.iter().all(|value| value.is_finite()),
            "non-finite genome weight"
        );
        Ok(())
    }
}

impl<C: Context + ?Sized> RandomInit<C> for RnnGenome {
    type Config = Initialization;

    fn random(config: &Initialization, cx: &mut C) -> Result<Self> {
        config.validate()?;
        let count = weight_count(config.hidden_units)?;
        let mut weights = Vec::new();
        weights.try_reserve_exact(count)?;
        if config.weight_min == config.weight_max {
            weights.resize(count, config.weight_min);
        } else {
            weights.extend(
                (0..count).map(|_| cx.rng().random_range(config.weight_min..config.weight_max)),
            );
        }
        Ok(Self {
            hidden_units: config.hidden_units,
            weights,
        })
    }
}

impl<C: Context + ?Sized> Genome<C> for RnnGenome {}

/// Independent uniform noise in [-max_delta, max_delta) for each weight and bias.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeightMutation {
    pub max_delta: f32,
}

impl Default for WeightMutation {
    fn default() -> Self {
        Self { max_delta: 0.02 }
    }
}

impl WeightMutation {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.max_delta >= 0.0 && (2.0 * self.max_delta).is_finite(),
            "mutation max_delta must be nonnegative with a finite sampling range"
        );
        Ok(())
    }
}

impl<C: Context + ?Sized> Mutate<C, WeightMutation> for RnnGenome {
    fn mutate(&mut self, param: &WeightMutation, cx: &mut C) -> Result<()> {
        let delta = param.max_delta;
        param.validate()?;
        self.validate()?;
        if delta > 0.0 {
            for weight in &mut self.weights {
                *weight += cx.rng().random_range(-delta..delta);
            }
        }
        Ok(())
    }
}
