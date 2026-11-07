use anyhow::{Result, ensure};
use rand::RngExt;
use rand_distr::StandardNormal;

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
    /// Initial inherited standard deviation of Gaussian weight/bias mutations.
    pub mutation_sigma: f32,
}

impl Default for Initialization {
    fn default() -> Self {
        Self {
            hidden_units: 4,
            weight_min: -0.5,
            weight_max: 0.5,
            // Match the variance of the previous uniform [-0.02, 0.02) noise.
            mutation_sigma: 0.02 / 3.0_f32.sqrt(),
        }
    }
}

impl Initialization {
    pub fn validate(&self) -> Result<()> {
        weight_count(self.hidden_units)?;
        validate_sigma(self.mutation_sigma)?;
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

fn validate_sigma(sigma: f32) -> Result<()> {
    ensure!(
        sigma.is_finite() && sigma > 0.0,
        "mutation_sigma must be finite and positive"
    );
    Ok(())
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
/// and output biases. Width and mutation scale are inherited with the weights.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RnnGenome {
    pub hidden_units: usize,
    pub weights: Vec<f32>,
    pub mutation_sigma: f32,
}

impl Default for RnnGenome {
    fn default() -> Self {
        let initialization = Initialization::default();
        let hidden_units = initialization.hidden_units;
        Self {
            hidden_units,
            weights: vec![0.0; weight_count(hidden_units).expect("default shape is valid")],
            mutation_sigma: initialization.mutation_sigma,
        }
    }
}

impl RnnGenome {
    pub fn validate(&self) -> Result<()> {
        validate_sigma(self.mutation_sigma)?;
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
            mutation_sigma: config.mutation_sigma,
        })
    }
}

impl<C: Context + ?Sized> Genome<C> for RnnGenome {}

/// Mutate log(sigma) first, then apply independent Gaussian weight/bias noise.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeightMutation {
    pub enabled: bool,
    /// Standard deviation of the additive change to ln(mutation_sigma).
    /// Zero keeps the inherited scale fixed while weights still mutate.
    pub log_sigma_std: f32,
}

impl Default for WeightMutation {
    fn default() -> Self {
        Self {
            enabled: true,
            log_sigma_std: 0.1,
        }
    }
}

impl WeightMutation {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.log_sigma_std.is_finite() && self.log_sigma_std >= 0.0,
            "log_sigma_std must be finite and nonnegative"
        );
        Ok(())
    }
}

impl<C: Context + ?Sized> Mutate<C, WeightMutation> for RnnGenome {
    fn mutate(&mut self, param: &WeightMutation, cx: &mut C) -> Result<()> {
        param.validate()?;
        self.validate()?;
        if !param.enabled {
            return Ok(());
        }
        let sigma = if param.log_sigma_std == 0.0 {
            self.mutation_sigma
        } else {
            (self.mutation_sigma.ln()
                + param.log_sigma_std * cx.rng().sample::<f32, _>(StandardNormal))
            .exp()
        };
        validate_sigma(sigma)?;
        for weight in &mut self.weights {
            *weight += sigma * cx.rng().sample::<f32, _>(StandardNormal);
            ensure!(weight.is_finite(), "non-finite mutated genome weight");
        }
        self.mutation_sigma = sigma;
        Ok(())
    }
}
