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

    /// Append a silent neuron or remove one, preserving every surviving coordinate.
    fn resize_hidden(&mut self, hidden: usize, removed: Option<usize>) -> Result<()> {
        let old = self.hidden_units;
        let mut weights = Vec::new();
        weights.try_reserve_exact(weight_count(hidden)?)?;
        weights.resize(weight_count(hidden)?, 0.0);
        let source = |i: usize| {
            let index = i + usize::from(removed.is_some_and(|r| i >= r));
            (index < old).then_some(index)
        };
        let old_recurrent = INPUTS * old;
        let recurrent = INPUTS * hidden;
        let old_bias = old_recurrent + old * old;
        let bias = recurrent + hidden * hidden;
        let old_output = old_bias + old;
        let output = bias + hidden;
        for i in 0..hidden {
            let Some(previous) = source(i) else { continue };
            weights[i * INPUTS..(i + 1) * INPUTS]
                .copy_from_slice(&self.weights[previous * INPUTS..(previous + 1) * INPUTS]);
            weights[bias + i] = self.weights[old_bias + previous];
            for j in 0..hidden {
                if let Some(previous_j) = source(j) {
                    weights[recurrent + i * hidden + j] =
                        self.weights[old_recurrent + previous * old + previous_j];
                }
            }
            for o in 0..OUTPUTS {
                weights[output + o * hidden + i] = self.weights[old_output + o * old + previous];
            }
        }
        weights[output + OUTPUTS * hidden..]
            .copy_from_slice(&self.weights[old_output + OUTPUTS * old..]);
        self.weights = weights;
        self.hidden_units = hidden;
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

/// Mutate log(sigma), attempt a size change, then apply Gaussian weight/bias noise.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Mutation {
    pub enabled: bool,
    /// Standard deviation of the additive change to ln(mutation_sigma).
    /// Zero keeps the inherited scale fixed while weights still mutate.
    pub log_sigma_std: f32,
    /// Probability of attempting a one-neuron size change, split equally by direction.
    pub size_probability: f32,
    pub min_hidden_units: usize,
    pub max_hidden_units: usize,
}

impl Default for Mutation {
    fn default() -> Self {
        Self {
            enabled: true,
            log_sigma_std: 0.1,
            size_probability: 0.01,
            min_hidden_units: 1,
            max_hidden_units: 32,
        }
    }
}

impl Mutation {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (0.0..=1.0).contains(&self.size_probability),
            "size_probability must be between zero and one"
        );
        weight_count(self.min_hidden_units)?;
        weight_count(self.max_hidden_units)?;
        ensure!(
            self.min_hidden_units <= self.max_hidden_units,
            "hidden size bounds are reversed"
        );
        ensure!(
            self.log_sigma_std.is_finite() && self.log_sigma_std >= 0.0,
            "log_sigma_std must be finite and nonnegative"
        );
        Ok(())
    }

    pub fn validate_width(&self, hidden: usize) -> Result<()> {
        self.validate()?;
        if self.enabled && self.size_probability > 0.0 {
            ensure!(
                (self.min_hidden_units..=self.max_hidden_units).contains(&hidden),
                "hidden width is outside the size mutation bounds"
            );
        }
        Ok(())
    }
}

impl<C: Context + ?Sized> Mutate<C, Mutation> for RnnGenome {
    fn mutate(&mut self, param: &Mutation, cx: &mut C) -> Result<()> {
        param.validate_width(self.hidden_units)?;
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
        if param.size_probability > 0.0 && cx.rng().random_bool(f64::from(param.size_probability)) {
            if cx.rng().random_bool(0.5) {
                if self.hidden_units < param.max_hidden_units {
                    self.resize_hidden(self.hidden_units + 1, None)?;
                }
            } else if self.hidden_units > param.min_hidden_units {
                let removed = cx.rng().random_range(0..self.hidden_units);
                self.resize_hidden(self.hidden_units - 1, Some(removed))?;
            }
        }
        for weight in &mut self.weights {
            *weight += sigma * cx.rng().sample::<f32, _>(StandardNormal);
            ensure!(weight.is_finite(), "non-finite mutated genome weight");
        }
        self.mutation_sigma = sigma;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Cpu;
    use rand::SeedableRng;
    use rand_xoshiro::Xoshiro256PlusPlus;

    fn numbered(hidden_units: usize) -> RnnGenome {
        RnnGenome {
            hidden_units,
            weights: (1..=weight_count(hidden_units).unwrap())
                .map(|i| i as f32)
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn growth_preserves_connections_and_adds_only_zero_coordinates() {
        let mut genome = numbered(2);
        genome.resize_hidden(3, None).unwrap();
        assert_eq!(genome.hidden_units, 3);
        assert_eq!(
            genome.weights,
            [
                1., 2., 3., 4., 5., 6., 7., 8., 9., 10., 11., 12., 0., 0., 0., 0., 0., 0., 13.,
                14., 0., 15., 16., 0., 0., 0., 0., 17., 18., 0., 19., 20., 0., 21., 22., 0., 23.,
                24.,
            ]
        );
        genome.validate().unwrap();
    }

    #[test]
    fn deleting_a_middle_neuron_preserves_surviving_connections_and_biases() {
        let mut genome = numbered(3);
        genome.resize_hidden(2, Some(1)).unwrap();
        assert_eq!(genome.hidden_units, 2);
        assert_eq!(
            genome.weights,
            [
                1., 2., 3., 4., 5., 6., 13., 14., 15., 16., 17., 18., 19., 21., 25., 27., 28., 30.,
                31., 33., 34., 36., 37., 38.,
            ]
        );
        genome.validate().unwrap();
    }

    #[test]
    fn size_attempts_are_symmetric_and_skip_changes_beyond_bounds() {
        let mut cx = Cpu {
            rng: Xoshiro256PlusPlus::seed_from_u64(91),
        };
        let mutation = Mutation {
            log_sigma_std: 0.0,
            size_probability: 1.0,
            min_hidden_units: 1,
            max_hidden_units: 3,
            ..Default::default()
        };
        for hidden in 1..=3 {
            let parent = RnnGenome {
                weights: vec![0.0; weight_count(hidden).unwrap()],
                hidden_units: hidden,
                ..Default::default()
            };
            let mut widths = [0; 3];
            for _ in 0..1000 {
                let mut child = parent.clone();
                child.mutate(&mutation, &mut cx).unwrap();
                child.validate().unwrap();
                assert!(child.weights.iter().all(|&w| w != 0.0));
                widths[child.hidden_units - 1] += 1;
            }
            assert_eq!(widths[if hidden == 2 { 1 } else { 3 - hidden }], 0);
            for count in widths.into_iter().filter(|&count| count > 0) {
                assert!((400..600).contains(&count));
            }
        }
    }

    #[test]
    fn invalid_size_settings_fail_before_mutation_or_random_draws() {
        let mut cx = Cpu {
            rng: Xoshiro256PlusPlus::seed_from_u64(12),
        };
        let before = cx.clone();
        let original = RnnGenome::default();
        for mutation in [
            Mutation {
                size_probability: -0.1,
                ..Default::default()
            },
            Mutation {
                size_probability: 1.1,
                ..Default::default()
            },
            Mutation {
                size_probability: f32::NAN,
                ..Default::default()
            },
            Mutation {
                min_hidden_units: 0,
                ..Default::default()
            },
            Mutation {
                max_hidden_units: usize::MAX,
                ..Default::default()
            },
            Mutation {
                min_hidden_units: 5,
                max_hidden_units: 4,
                ..Default::default()
            },
            Mutation {
                max_hidden_units: 3,
                ..Default::default()
            },
        ] {
            let mut genome = original.clone();
            assert!(genome.mutate(&mutation, &mut cx).is_err());
            assert_eq!(genome, original);
            assert_eq!(cx, before);
        }
    }
}
