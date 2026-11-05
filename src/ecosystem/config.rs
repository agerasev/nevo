//! Initialization and component metrics settings; presets live in `configs/`.

use std::{fmt, str::FromStr};

use anyhow::{Result, ensure};
use glam::DVec2;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::{Admission, HALF_SIZE, START_ENERGY};
use crate::{
    metrics::Selection,
    mind::{animal, rnn},
};

/// Random placement or an explicit world-space point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Placement {
    #[default]
    Random,
    Fixed(DVec2),
}

impl Placement {
    pub fn position(self) -> Option<DVec2> {
        match self {
            Self::Random => None,
            Self::Fixed(position) => Some(position),
        }
    }

    fn validate(self) -> Result<()> {
        if let Some(position) = self.position() {
            ensure!(
                position.is_finite() && position.abs().cmple(HALF_SIZE).all(),
                "initial position must be finite and inside the arena"
            );
        }
        Ok(())
    }
}

impl fmt::Display for Placement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Random => f.write_str("random"),
            Self::Fixed(p) => write!(f, "{},{}", p.x, p.y),
        }
    }
}

impl FromStr for Placement {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        if value == "random" {
            return Ok(Self::Random);
        }
        let (x, y) = value
            .split_once(',')
            .ok_or_else(|| anyhow::anyhow!("expected random or X,Y"))?;
        Ok(Self::Fixed(DVec2::new(
            x.trim().parse()?,
            y.trim().parse()?,
        )))
    }
}

impl Serialize for Placement {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Random => serializer.serialize_str("random"),
            Self::Fixed(p) => p.to_array().serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Placement {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Value {
            Point([f64; 2]),
            Text(String),
        }
        match Value::deserialize(deserializer)? {
            Value::Point(point) => Ok(Self::Fixed(DVec2::from_array(point))),
            Value::Text(text) => text.parse().map_err(de::Error::custom),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "MC: Deserialize<'de> + Default")
)]
pub struct Config<MC = animal::Config<rnn::Config>> {
    /// Number of animals admitted at initialization.
    pub initial_animals: usize,

    /// Number of plants, replenished throughout the run.
    pub plants: usize,

    /// Population floor maintained by immigration; zero disables it.
    pub minimum_population: usize,

    /// Starting energy of initial animals; later immigrants use the model default.
    pub initial_animal_energy: f64,

    /// Initial animal placement: random or a fixed world-space point.
    pub initial_animal_position: Placement,

    /// Initial plant placement; replacement plants remain random.
    pub initial_plant_position: Placement,

    pub metrics: MetricsConfig,
    pub animal: AnimalConfig<MC>,
    pub plant: PlantConfig,
}

impl<MC: Default> Default for Config<MC> {
    fn default() -> Self {
        Self {
            initial_animals: 20,
            plants: 500,
            minimum_population: 20,
            initial_animal_energy: START_ENERGY,
            initial_animal_position: Placement::Random,
            initial_plant_position: Placement::Random,
            metrics: MetricsConfig::default(),
            animal: AnimalConfig::default(),
            plant: PlantConfig::default(),
        }
    }
}

impl<MC> Config<MC> {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.metrics.enabled || self.metrics.window_ticks > 0,
            "metrics window_ticks must be positive"
        );
        ensure!(
            self.initial_animal_energy.is_finite(),
            "initial animal energy must be finite"
        );
        self.initial_animal_position.validate()?;
        self.initial_plant_position.validate()
    }

    /// Metrics settings may change when a checkpoint is resumed.
    pub fn same_initialization<Other>(&self, other: &Config<Other>) -> bool {
        self.initial_animals == other.initial_animals
            && self.plants == other.plants
            && self.minimum_population == other.minimum_population
            && self.initial_animal_energy == other.initial_animal_energy
            && self.initial_animal_position == other.initial_animal_position
            && self.initial_plant_position == other.initial_plant_position
    }

    pub(super) fn admission(&self) -> Admission {
        Admission {
            position: self.initial_animal_position.position(),
            energy: self.initial_animal_energy,
        }
    }
}

/// Enables collection/output windows; component selectors choose their contents.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub window_ticks: u64,
    pub population: bool,
    pub event_rates: bool,
    pub genetic_diversity: bool,
}
impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_ticks: 100,
            population: true,
            event_rates: true,
            genetic_diversity: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnimalConfig<MC = animal::Config<rnn::Config>> {
    pub metrics: AnimalMetricsConfig,
    pub body: BodyConfig,
    pub mind: MC,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnimalMetricsConfig {
    /// Recent totals per currently present animal, not lifetime fitness.
    pub food_gained: Selection,
    pub offspring: Selection,
    pub distance: Selection,
    /// Active ticks observed in this window for animals that died.
    pub observed_survival: Selection,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodyConfig {
    pub metrics: BodyMetricsConfig,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodyMetricsConfig {
    #[serde(deserialize_with = "deserialize_energy_selection")]
    pub energy: Selection,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlantConfig {
    pub metrics: PlantMetricsConfig,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlantMetricsConfig {
    #[serde(deserialize_with = "deserialize_energy_selection")]
    pub energy: Selection,
}

fn default_energy_selection() -> Selection {
    Selection {
        mean: true,
        min: true,
        max: true,
        ..Selection::default()
    }
}

impl Default for BodyMetricsConfig {
    fn default() -> Self {
        Self {
            energy: default_energy_selection(),
        }
    }
}

impl Default for PlantMetricsConfig {
    fn default() -> Self {
        Self {
            energy: default_energy_selection(),
        }
    }
}

// A partially specified energy table overrides the lightweight field defaults.
fn deserialize_energy_selection<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Selection, D::Error> {
    fn enabled() -> bool {
        true
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fields {
        #[serde(default)]
        count: bool,
        #[serde(default)]
        sum: bool,
        #[serde(default = "enabled")]
        mean: bool,
        #[serde(default = "enabled")]
        min: bool,
        #[serde(default = "enabled")]
        max: bool,
        #[serde(default)]
        variance: bool,
    }
    let fields = Fields::deserialize(deserializer)?;
    Ok(Selection {
        count: fields.count,
        sum: fields.sum,
        mean: fields.mean,
        min: fields.min,
        max: fields.max,
        variance: fields.variance,
    })
}
