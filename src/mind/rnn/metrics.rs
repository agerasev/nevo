use anyhow::{Result, ensure};
use serde::{Serialize, Serializer};

use crate::metrics::{Aggregate, Metrics, Moments, SelectedStats, Selection};

use super::RnnGenome;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    pub activation: Selection,
    /// Fraction of hidden units with |activation| >= saturation_threshold.
    pub saturation: bool,
    pub saturation_threshold: f32,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            activation: Selection::default(),
            saturation: false,
            saturation_threshold: 0.95,
        }
    }
}

impl MetricsConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (0.0..=1.0).contains(&self.saturation_threshold),
            "saturation_threshold must be between zero and one"
        );
        Ok(())
    }
}

/// Current hidden activations, merged across brains, rather than across ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Summary {
    activation: SelectedStats,
    saturation_enabled: bool,
    saturated: u64,
    units: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Statistics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation: Option<crate::metrics::Statistics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saturation_fraction: Option<f64>,
}

impl Summary {
    pub(super) fn from_memory(memory: &[f32], config: &MetricsConfig) -> Self {
        let mut summary = Self {
            activation: SelectedStats::new(config.activation),
            saturation_enabled: config.saturation,
            ..Self::default()
        };
        if config.activation.enabled() {
            for value in memory {
                summary.activation.push(f64::from(*value));
            }
        }
        if config.saturation {
            summary.units = memory.len() as u64;
            summary.saturated = memory
                .iter()
                .filter(|value| value.abs() >= config.saturation_threshold)
                .count() as u64;
        }
        summary
    }

    pub fn merge(&mut self, other: &Self) {
        self.activation.merge(&other.activation);
        self.saturation_enabled |= other.saturation_enabled;
        self.saturated += other.saturated;
        self.units += other.units;
    }

    pub fn value(&self) -> Statistics {
        Statistics {
            activation: self.activation.enabled().then(|| self.activation.value()),
            saturation_fraction: (self.saturation_enabled && self.units > 0)
                .then(|| self.saturated as f64 / self.units as f64),
        }
    }
}

impl Serialize for Summary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value().serialize(serializer)
    }
}

/// Accumulate each weight coordinate separately: differences between coordinates
/// within a single genome are not population diversity. Mixed shapes retain the
/// genome count, but have no common-coordinate variance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GenomeSummary {
    coordinates: Vec<Moments>,
    hidden_units: Option<usize>,
    genomes: u64,
    incompatible_shapes: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct GenomeStatistics {
    pub genomes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_coordinate_variance: Option<f64>,
}

impl GenomeSummary {
    pub fn merge(&mut self, other: &Self) {
        if other.genomes == 0 {
            return;
        }
        if self.genomes == 0 {
            self.clone_from(other);
            return;
        }
        self.genomes += other.genomes;
        self.incompatible_shapes |= other.incompatible_shapes
            || self.hidden_units != other.hidden_units
            || self.coordinates.len() != other.coordinates.len();
        if self.incompatible_shapes {
            return;
        }
        for (ours, theirs) in self.coordinates.iter_mut().zip(&other.coordinates) {
            ours.merge(theirs);
        }
    }

    pub fn value(&self) -> GenomeStatistics {
        GenomeStatistics {
            genomes: self.genomes,
            mean_coordinate_variance: (self.genomes > 0 && !self.incompatible_shapes).then(|| {
                self.coordinates
                    .iter()
                    .map(|value| value.variance().unwrap())
                    .sum::<f64>()
                    / self.coordinates.len() as f64
            }),
        }
    }
}

impl Serialize for GenomeSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value().serialize(serializer)
    }
}

impl Metrics for RnnGenome {
    type Summary = GenomeSummary;

    fn collect_metrics(&self, summary: &mut GenomeSummary) {
        if summary.genomes == 0 {
            summary.hidden_units = Some(self.hidden_units);
            summary.coordinates = vec![Moments::default(); self.weights.len()];
        }
        summary.genomes += 1;
        summary.incompatible_shapes |= summary.hidden_units != Some(self.hidden_units)
            || summary.coordinates.len() != self.weights.len()
            || self.validate().is_err();
        if summary.incompatible_shapes {
            return;
        }
        for (coordinate, weight) in summary.coordinates.iter_mut().zip(&self.weights) {
            coordinate.push(f64::from(*weight));
        }
    }
}
