//! Parameters for initializing the ecosystem; presets live in `configs/`.

use std::{fmt, str::FromStr};

use anyhow::{Result, ensure};
use glam::DVec2;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::{Admission, HALF_SIZE, START_ENERGY};

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
#[serde(default, deny_unknown_fields)]
pub struct Config {
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            initial_animals: 20,
            plants: 500,
            minimum_population: 20,
            initial_animal_energy: START_ENERGY,
            initial_animal_position: Placement::Random,
            initial_plant_position: Placement::Random,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.initial_animal_energy.is_finite(),
            "initial animal energy must be finite"
        );
        self.initial_animal_position.validate()?;
        self.initial_plant_position.validate()
    }

    pub(super) fn admission(&self) -> Admission {
        Admission {
            position: self.initial_animal_position.position(),
            energy: self.initial_animal_energy,
        }
    }
}
