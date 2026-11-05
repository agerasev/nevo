//! Physical, genetic, admission, and metric settings; presets live in `configs/`.

use std::{fmt, str::FromStr};

use anyhow::{Result, ensure};
use glam::DVec2;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

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

    fn validate(self, world: WorldConfig) -> Result<()> {
        if let Some(position) = self.position() {
            ensure!(
                position.is_finite() && position.abs().cmple(world.half_size()).all(),
                "position must be finite and inside the arena"
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
    bound(
        deserialize = "MC: Deserialize<'de> + Default, IC: Deserialize<'de> + Default, P: Deserialize<'de> + Default"
    )
)]
pub struct Config<
    MC = animal::Config<rnn::Config>,
    IC = rnn::Initialization,
    P = rnn::WeightMutation,
> {
    /// Size of the first batch of immigrants.
    pub initial_animals: usize,
    pub plants: usize,
    /// Population floor maintained by later immigration; zero disables it.
    pub minimum_population: usize,
    pub world: WorldConfig,
    pub immigration: ImmigrationConfig,
    pub sensing: SensingConfig,
    pub genetics: GeneticsConfig<IC, P>,
    pub metrics: MetricsConfig,
    pub animal: AnimalConfig<MC>,
    pub plant: PlantConfig,
}

impl<MC: Default, IC: Default, P: Default> Default for Config<MC, IC, P> {
    fn default() -> Self {
        Self {
            initial_animals: 20,
            plants: 500,
            minimum_population: 20,
            world: WorldConfig::default(),
            immigration: ImmigrationConfig::default(),
            sensing: SensingConfig::default(),
            genetics: GeneticsConfig::default(),
            metrics: MetricsConfig::default(),
            animal: AnimalConfig::default(),
            plant: PlantConfig::default(),
        }
    }
}

impl<MC, IC, P> Config<MC, IC, P> {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.metrics.enabled || self.metrics.window_ticks > 0,
            "metrics window_ticks must be positive"
        );
        self.parameters().validate()
    }

    pub(super) fn parameters(&self) -> Parameters {
        Parameters {
            world: self.world,
            immigration: self.immigration,
            sensing: self.sensing,
            body: self.animal.body.parameters(),
            reproduction: self.animal.reproduction,
            feeding: self.animal.feeding,
            plant: self.plant.parameters(),
        }
    }

    /// Runtime metrics may change on resume; model parameters must remain equal.
    pub fn same_model<Other>(&self, other: &Config<Other, IC, P>) -> bool
    where
        IC: PartialEq,
        P: PartialEq,
    {
        self.initial_animals == other.initial_animals
            && self.plants == other.plants
            && self.minimum_population == other.minimum_population
            && self.parameters() == other.parameters()
            && self.genetics == other.genetics
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldConfig {
    pub width: f64,
    pub height: f64,
}
impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 1260.0,
            height: 700.0,
        }
    }
}
impl WorldConfig {
    pub fn half_size(self) -> DVec2 {
        DVec2::new(self.width, self.height) * 0.5
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImmigrationConfig {
    pub energy: f64,
    pub position: Placement,
}
impl Default for ImmigrationConfig {
    fn default() -> Self {
        Self {
            energy: 100.0,
            position: Placement::Random,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensingConfig {
    /// Multiplier applied before dividing displacement by the world's half-height.
    pub distance_scale: f64,
    pub softening: f64,
    pub gradient_dead_zone: f64,
}
impl Default for SensingConfig {
    fn default() -> Self {
        Self {
            distance_scale: 0.5,
            softening: 0.01,
            gradient_dead_zone: 0.01,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeneticsConfig<IC = rnn::Initialization, P = rnn::WeightMutation> {
    pub initialization: IC,
    pub mutation: P,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReproductionConfig {
    pub threshold: f64,
    pub child_energy_fraction: f64,
    pub separation_scale: f64,
}
impl Default for ReproductionConfig {
    fn default() -> Self {
        Self {
            threshold: 400.0,
            child_energy_fraction: 0.5,
            separation_scale: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeedingConfig {
    pub efficiency: f64,
    pub contact_scale: f64,
}
impl Default for FeedingConfig {
    fn default() -> Self {
        Self {
            efficiency: 0.1,
            contact_scale: 0.8,
        }
    }
}

/// Canonical physical rules, independent of omitted runtime metric selectors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Parameters {
    pub world: WorldConfig,
    pub immigration: ImmigrationConfig,
    pub sensing: SensingConfig,
    pub body: BodyParameters,
    pub reproduction: ReproductionConfig,
    pub feeding: FeedingConfig,
    pub plant: PlantParameters,
}
impl Parameters {
    pub fn validate(self) -> Result<()> {
        fn nonnegative(value: f64, name: &str) -> Result<()> {
            ensure!(
                value.is_finite() && value >= 0.0,
                "{name} must be finite and nonnegative"
            );
            Ok(())
        }
        fn positive(value: f64, name: &str) -> Result<()> {
            ensure!(
                value.is_finite() && value > 0.0,
                "{name} must be finite and positive"
            );
            Ok(())
        }
        positive(self.world.half_size().x, "world width")?;
        positive(self.world.half_size().y, "world height")?;
        nonnegative(self.immigration.energy, "immigration energy")?;
        self.immigration.position.validate(self.world)?;
        positive(self.sensing.distance_scale, "sensing distance_scale")?;
        positive(
            self.sensing.distance_scale / self.world.half_size().y,
            "normalized sensing scale",
        )?;
        positive(self.sensing.softening, "sensing softening")?;
        positive(self.sensing.softening.powi(3), "cubed sensing softening")?;
        nonnegative(
            self.sensing.gradient_dead_zone,
            "sensing gradient_dead_zone",
        )?;
        nonnegative(self.body.upkeep, "animal upkeep")?;
        nonnegative(
            self.body.movement_cost_per_distance,
            "animal movement_cost_per_distance",
        )?;
        nonnegative(self.body.max_speed, "animal max_speed")?;
        nonnegative(self.body.movement_time_step, "animal movement_time_step")?;
        nonnegative(
            self.body.max_speed * self.body.movement_time_step,
            "maximum movement",
        )?;
        nonnegative(
            self.body.upkeep
                + self.body.movement_cost_per_distance
                    * (self.body.max_speed * self.body.movement_time_step),
            "maximum movement cost plus upkeep",
        )?;
        nonnegative(self.body.movement_dead_zone, "animal movement_dead_zone")?;
        nonnegative(self.body.radius_scale, "animal radius_scale")?;
        positive(self.reproduction.threshold, "reproduction threshold")?;
        let fraction = self.reproduction.child_energy_fraction;
        ensure!(
            fraction.is_finite() && fraction > 0.0 && fraction < 1.0,
            "child_energy_fraction must be strictly between zero and one"
        );
        nonnegative(
            self.reproduction.separation_scale,
            "reproduction separation_scale",
        )?;
        ensure!(
            self.feeding.efficiency.is_finite() && (0.0..=1.0).contains(&self.feeding.efficiency),
            "feeding efficiency must be between zero and one"
        );
        nonnegative(self.feeding.contact_scale, "feeding contact_scale")?;
        positive(self.plant.max_energy, "plant max_energy")?;
        for energy in [self.plant.initial_energy, self.plant.replacement_energy] {
            ensure!(
                energy.is_finite() && (0.0..=self.plant.max_energy).contains(&energy),
                "plant admission energy must be between zero and max_energy"
            );
        }
        nonnegative(self.plant.growth, "plant growth")?;
        nonnegative(
            self.plant.max_energy + self.plant.growth,
            "plant energy after growth",
        )?;
        nonnegative(self.plant.radius_scale, "plant radius_scale")?;
        nonnegative(
            self.plant.radius_scale * self.plant.max_energy.sqrt(),
            "maximum plant radius",
        )?;
        nonnegative(
            self.body.radius_scale * self.immigration.energy.sqrt(),
            "immigrant radius",
        )?;
        self.plant.initial_position.validate(self.world)?;
        self.plant.replacement_position.validate(self.world)
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
    pub reproduction: ReproductionConfig,
    pub feeding: FeedingConfig,
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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodyConfig {
    pub upkeep: f64,
    /// Energy per unit of controlled distance after boundary clamping.
    pub movement_cost_per_distance: f64,
    pub max_speed: f64,
    /// Scales movement only; energy rates are per tick.
    pub movement_time_step: f64,
    pub movement_dead_zone: f64,
    pub radius_scale: f64,
    pub metrics: BodyMetricsConfig,
}
impl Default for BodyConfig {
    fn default() -> Self {
        let parameters = BodyParameters::default();
        Self {
            upkeep: parameters.upkeep,
            movement_cost_per_distance: parameters.movement_cost_per_distance,
            max_speed: parameters.max_speed,
            movement_time_step: parameters.movement_time_step,
            movement_dead_zone: parameters.movement_dead_zone,
            radius_scale: parameters.radius_scale,
            metrics: BodyMetricsConfig::default(),
        }
    }
}
impl BodyConfig {
    pub(super) fn parameters(self) -> BodyParameters {
        BodyParameters {
            upkeep: self.upkeep,
            movement_cost_per_distance: self.movement_cost_per_distance,
            max_speed: self.max_speed,
            movement_time_step: self.movement_time_step,
            movement_dead_zone: self.movement_dead_zone,
            radius_scale: self.radius_scale,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BodyParameters {
    pub upkeep: f64,
    pub movement_cost_per_distance: f64,
    pub max_speed: f64,
    /// Scales movement only; energy rates are per tick.
    pub movement_time_step: f64,
    pub movement_dead_zone: f64,
    pub radius_scale: f64,
}
impl Default for BodyParameters {
    fn default() -> Self {
        Self {
            upkeep: 2.0,
            movement_cost_per_distance: 0.0,
            max_speed: 100.0,
            movement_time_step: 0.1,
            movement_dead_zone: 0.01,
            radius_scale: 0.5,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodyMetricsConfig {
    #[serde(deserialize_with = "deserialize_energy_selection")]
    pub energy: Selection,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlantConfig {
    pub initial_energy: f64,
    pub replacement_energy: f64,
    pub growth: f64,
    pub max_energy: f64,
    pub radius_scale: f64,
    pub initial_position: Placement,
    pub replacement_position: Placement,
    pub metrics: PlantMetricsConfig,
}
impl Default for PlantConfig {
    fn default() -> Self {
        let parameters = PlantParameters::default();
        Self {
            initial_energy: parameters.initial_energy,
            replacement_energy: parameters.replacement_energy,
            growth: parameters.growth,
            max_energy: parameters.max_energy,
            radius_scale: parameters.radius_scale,
            initial_position: parameters.initial_position,
            replacement_position: parameters.replacement_position,
            metrics: PlantMetricsConfig::default(),
        }
    }
}
impl PlantConfig {
    pub(super) fn parameters(self) -> PlantParameters {
        PlantParameters {
            initial_energy: self.initial_energy,
            replacement_energy: self.replacement_energy,
            growth: self.growth,
            max_energy: self.max_energy,
            radius_scale: self.radius_scale,
            initial_position: self.initial_position,
            replacement_position: self.replacement_position,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlantParameters {
    pub initial_energy: f64,
    pub replacement_energy: f64,
    pub growth: f64,
    pub max_energy: f64,
    pub radius_scale: f64,
    pub initial_position: Placement,
    pub replacement_position: Placement,
}
impl Default for PlantParameters {
    fn default() -> Self {
        Self {
            initial_energy: 400.0,
            replacement_energy: 0.0,
            growth: 1.0,
            max_energy: 400.0,
            radius_scale: 0.5,
            initial_position: Placement::Random,
            replacement_position: Placement::Random,
        }
    }
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
