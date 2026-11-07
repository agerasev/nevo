//! Physical, genetic, admission, and metric settings; presets live in `configs/`.

use std::{fmt, str::FromStr};

use anyhow::{Result, ensure};
use glam::Vec2;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::mind::{animal, rnn};

/// Random placement or an explicit world-space point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Placement {
    #[default]
    Random,
    Fixed(Vec2),
}

impl Placement {
    pub fn position(self) -> Option<Vec2> {
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
        Ok(Self::Fixed(Vec2::new(x.trim().parse()?, y.trim().parse()?)))
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
            Point([f32; 2]),
            Text(String),
        }
        match Value::deserialize(deserializer)? {
            Value::Point(point) => Ok(Self::Fixed(Vec2::from_array(point))),
            Value::Text(text) => text.parse().map_err(de::Error::custom),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(
        deserialize = "MC: Deserialize<'de> + Default, IC: Deserialize<'de> + Default, P: Deserialize<'de> + Default"
    )
)]
pub struct Config<MC = animal::Config<rnn::Config>, IC = rnn::Initialization, P = rnn::Mutation> {
    /// Size of the first batch of immigrants.
    pub initial_animals: usize,
    /// Initial count; also the maintained count when replenishment is omitted.
    pub plants: usize,
    /// Population floor maintained by later immigration; zero disables it.
    pub minimum_population: usize,
    pub world: WorldConfig,
    pub physics: PhysicsConfig,
    pub obstacles: super::ObstacleConfig,
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
            physics: PhysicsConfig::default(),
            obstacles: super::ObstacleConfig::default(),
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
        if let Some(replenishment) = self.plant.replenishment
            && self.plant.initial_position == Placement::Random
        {
            ensure!(
                self.plants <= replenishment.max_initial_attempts,
                "initial plant count exceeds max_initial_attempts"
            );
        }
        self.parameters().validate()?;
        self.obstacles.validate(self.world)
    }

    pub(super) fn parameters(&self) -> Parameters {
        Parameters {
            world: self.world,
            physics: self.physics,
            immigration: self.immigration,
            sensing: self.sensing,
            body: self.animal.body.parameters(),
            brain_weight_upkeep: self.animal.brain_weight_upkeep,
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
            && self.obstacles == other.obstacles
            && self.genetics == other.genetics
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorldConfig {
    pub width: f32,
    pub height: f32,
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
    pub fn half_size(self) -> Vec2 {
        Vec2::new(self.width, self.height) * 0.5
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhysicsConfig {
    /// Physical time per ecological tick; viewer pacing does not change it.
    pub time_step: f32,
    /// Fixed RK4 steps per ecological tick, each lasting time_step / substeps.
    pub substeps: u32,
    /// Common inertial mass per unit of stored animal or plant energy.
    pub mass_per_energy: f32,
    /// Natural angular frequency of normal contact repulsion.
    pub contact_frequency: f32,
    /// Contact damping ratio; one is critical damping for an isolated pair.
    pub contact_damping: f32,
    /// Bounded random placement attempts for each immigrant or fixed-count plant.
    pub max_placement_attempts: u32,
}
impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            time_step: 0.1,
            substeps: 4,
            mass_per_energy: 1.0,
            contact_frequency: 20.0,
            contact_damping: 1.0,
            max_placement_attempts: 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImmigrationConfig {
    pub energy: f32,
    pub position: Placement,
}
impl Default for ImmigrationConfig {
    fn default() -> Self {
        Self {
            energy: 0.25,
            position: Placement::Random,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SensingConfig {
    /// Inverse world-unit scale applied to displacement from each food source.
    pub distance_scale: f32,
    /// Gain on summed strength and gradient, before the gradient dead zone.
    pub signal_scale: f32,
    pub softening: f32,
    pub gradient_dead_zone: f32,
    /// Gain on body-relative linear velocity before neural encoding.
    pub velocity_scale: f32,
    /// Gain on angular velocity before neural encoding.
    pub angular_velocity_scale: f32,
}
impl Default for SensingConfig {
    fn default() -> Self {
        Self {
            distance_scale: 1.0 / 700.0,
            signal_scale: 1.0 / 500.0,
            softening: 0.01,
            gradient_dead_zone: 0.01,
            velocity_scale: 0.01,
            angular_velocity_scale: 0.2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GeneticsConfig<IC = rnn::Initialization, P = rnn::Mutation> {
    pub initialization: IC,
    pub mutation: P,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ReproductionConfig {
    pub threshold: f32,
    pub child_energy_fraction: f32,
    pub separation_scale: f32,
}
impl Default for ReproductionConfig {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            child_energy_fraction: 0.5,
            separation_scale: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeedingConfig {
    pub efficiency: f32,
}
impl Default for FeedingConfig {
    fn default() -> Self {
        Self { efficiency: 0.1 }
    }
}

/// Canonical physical rules, independent of omitted runtime metric selectors.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Parameters {
    pub world: WorldConfig,
    pub physics: PhysicsConfig,
    pub immigration: ImmigrationConfig,
    pub sensing: SensingConfig,
    pub body: BodyParameters,
    pub brain_weight_upkeep: f32,
    pub reproduction: ReproductionConfig,
    pub feeding: FeedingConfig,
    pub plant: PlantParameters,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            world: WorldConfig::default(),
            physics: PhysicsConfig::default(),
            immigration: ImmigrationConfig::default(),
            sensing: SensingConfig::default(),
            body: BodyParameters::default(),
            brain_weight_upkeep: AnimalConfig::<()>::default().brain_weight_upkeep,
            reproduction: ReproductionConfig::default(),
            feeding: FeedingConfig::default(),
            plant: PlantParameters::default(),
        }
    }
}
impl Parameters {
    pub fn validate(self) -> Result<()> {
        fn nonnegative(value: f32, name: &str) -> Result<()> {
            ensure!(
                value.is_finite() && value >= 0.0,
                "{name} must be finite and nonnegative"
            );
            Ok(())
        }
        fn positive(value: f32, name: &str) -> Result<()> {
            ensure!(
                value.is_finite() && value > 0.0,
                "{name} must be finite and positive"
            );
            Ok(())
        }
        positive(self.world.half_size().x, "world width")?;
        positive(self.world.half_size().y, "world height")?;
        positive(self.physics.time_step, "physics time_step")?;
        positive(self.physics.mass_per_energy, "physics mass_per_energy")?;
        positive(self.physics.contact_frequency, "physics contact_frequency")?;
        positive(
            self.physics.contact_frequency.powi(2),
            "squared contact_frequency",
        )?;
        nonnegative(self.physics.contact_damping, "physics contact_damping")?;
        nonnegative(
            2.0 * self.physics.contact_damping * self.physics.contact_frequency,
            "contact damping rate",
        )?;
        ensure!(
            self.physics.max_placement_attempts > 0,
            "max_placement_attempts must be positive"
        );
        ensure!(
            self.physics.substeps > 0,
            "physics substeps must be positive"
        );
        positive(
            self.physics.time_step / self.physics.substeps as f32,
            "physics substep duration",
        )?;
        nonnegative(self.immigration.energy, "immigration energy")?;
        self.immigration.position.validate(self.world)?;
        positive(self.sensing.distance_scale, "sensing distance_scale")?;
        positive(self.sensing.signal_scale, "sensing signal_scale")?;
        positive(self.sensing.velocity_scale, "sensing velocity_scale")?;
        positive(
            self.sensing.angular_velocity_scale,
            "sensing angular_velocity_scale",
        )?;
        positive(self.sensing.softening, "sensing softening")?;
        positive(self.sensing.softening.powi(3), "cubed sensing softening")?;
        nonnegative(
            self.sensing.gradient_dead_zone,
            "sensing gradient_dead_zone",
        )?;
        nonnegative(self.body.upkeep, "animal upkeep")?;
        nonnegative(self.brain_weight_upkeep, "brain weight upkeep")?;
        nonnegative(self.body.max_acceleration, "animal max_acceleration")?;
        nonnegative(
            self.body.max_angular_acceleration,
            "animal max_angular_acceleration",
        )?;
        nonnegative(self.body.linear_drag, "animal linear_drag")?;
        nonnegative(self.body.angular_drag, "animal angular_drag")?;
        nonnegative(self.plant.linear_drag, "plant linear_drag")?;
        nonnegative(self.plant.angular_drag, "plant angular_drag")?;
        for coefficient in [
            self.body.linear_drag,
            self.body.angular_drag,
            self.body.max_acceleration,
            self.body.max_angular_acceleration,
            self.plant.linear_drag,
            self.plant.angular_drag,
        ] {
            nonnegative(
                coefficient * self.physics.time_step,
                "integrated motion coefficient",
            )?;
        }
        nonnegative(self.body.energy_scale, "animal energy_scale")?;
        ensure!(
            self.body.motor_efficiency.is_finite()
                && self.body.motor_efficiency > 0.0
                && self.body.motor_efficiency <= 1.0,
            "animal motor_efficiency must be in (0, 1]"
        );
        nonnegative(
            self.body.energy_scale / self.body.motor_efficiency,
            "work energy conversion",
        )?;
        nonnegative(self.body.force_loss, "animal force_loss")?;
        nonnegative(self.body.torque_loss, "animal torque_loss")?;
        positive(self.body.radius_scale, "animal radius_scale")?;
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
        positive(self.plant.max_energy, "plant max_energy")?;
        for energy in [self.plant.initial_energy, self.plant.replacement_energy] {
            ensure!(
                energy.is_finite() && (0.0..=self.plant.max_energy).contains(&energy),
                "plant admission energy must be between zero and max_energy"
            );
        }
        nonnegative(self.plant.growth, "plant growth")?;
        nonnegative(
            self.plant.max_energy + self.plant.growth * self.plant.patches.growth_peak,
            "plant energy after growth",
        )?;
        positive(self.plant.radius_scale, "plant radius_scale")?;
        nonnegative(
            self.plant.radius_scale * self.plant.max_energy.sqrt(),
            "maximum plant radius",
        )?;
        nonnegative(
            self.body.radius_scale * self.immigration.energy.sqrt(),
            "immigrant radius",
        )?;
        self.plant.initial_position.validate(self.world)?;
        self.plant.replacement_position.validate(self.world)?;
        self.plant
            .patches
            .validate(self.world, self.plant.max_radius(), self.plant.growth)?;
        if let Some(replenishment) = self.plant.replenishment {
            replenishment.validate(self.world, self.plant.max_radius(), self.plant.patches)?;
            ensure!(
                self.plant.replacement_position == Placement::Random,
                "density-based replenishment requires random replacement_position"
            );
        }
        if self.plant.patches.count > 0 || self.plant.replenishment.is_some() {
            let limit = self.world.half_size() - Vec2::splat(self.plant.max_radius());
            for placement in [self.plant.initial_position, self.plant.replacement_position] {
                if let Some(position) = placement.position() {
                    ensure!(
                        position.abs().cmple(limit).all(),
                        "fixed plant positions must leave room for mature plants when patches or replenishment are enabled"
                    );
                }
            }
        }
        Ok(())
    }
}

/// Enables inexpensive ecological observations and their recording interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub window_ticks: u64,
}
impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            window_ticks: 1000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnimalConfig<MC = animal::Config<rnn::Config>> {
    pub body: BodyConfig,
    /// Reserve energy per stored brain weight or bias, once per ecological tick.
    pub brain_weight_upkeep: f32,
    pub reproduction: ReproductionConfig,
    pub feeding: FeedingConfig,
    pub mind: MC,
}

impl<MC: Default> Default for AnimalConfig<MC> {
    fn default() -> Self {
        Self {
            body: BodyConfig::default(),
            brain_weight_upkeep: 1e-6,
            reproduction: ReproductionConfig::default(),
            feeding: FeedingConfig::default(),
            mind: MC::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodyConfig {
    /// Reserve energy spent once per ecological tick.
    pub upkeep: f32,
    pub max_acceleration: f32,
    pub max_angular_acceleration: f32,
    /// Linear velocity decay rate (inverse physical time).
    pub linear_drag: f32,
    /// Angular velocity decay rate (inverse physical time).
    pub angular_drag: f32,
    /// Reserve-energy units per mechanical-energy unit; zero disables motor costs.
    pub energy_scale: f32,
    /// Fraction of supplied work converted to positive mechanical work.
    pub motor_efficiency: f32,
    /// Resistive power coefficient multiplying squared applied force.
    pub force_loss: f32,
    /// Resistive power coefficient multiplying squared applied torque.
    pub torque_loss: f32,
    pub radius_scale: f32,
}
impl Default for BodyConfig {
    fn default() -> Self {
        let parameters = BodyParameters::default();
        Self {
            upkeep: parameters.upkeep,
            max_acceleration: parameters.max_acceleration,
            max_angular_acceleration: parameters.max_angular_acceleration,
            linear_drag: parameters.linear_drag,
            angular_drag: parameters.angular_drag,
            energy_scale: parameters.energy_scale,
            motor_efficiency: parameters.motor_efficiency,
            force_loss: parameters.force_loss,
            torque_loss: parameters.torque_loss,
            radius_scale: parameters.radius_scale,
        }
    }
}
impl BodyConfig {
    pub(super) fn parameters(self) -> BodyParameters {
        BodyParameters {
            upkeep: self.upkeep,
            max_acceleration: self.max_acceleration,
            max_angular_acceleration: self.max_angular_acceleration,
            linear_drag: self.linear_drag,
            angular_drag: self.angular_drag,
            energy_scale: self.energy_scale,
            motor_efficiency: self.motor_efficiency,
            force_loss: self.force_loss,
            torque_loss: self.torque_loss,
            radius_scale: self.radius_scale,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BodyParameters {
    pub upkeep: f32,
    pub max_acceleration: f32,
    pub max_angular_acceleration: f32,
    pub linear_drag: f32,
    pub angular_drag: f32,
    pub energy_scale: f32,
    pub motor_efficiency: f32,
    pub force_loss: f32,
    pub torque_loss: f32,
    pub radius_scale: f32,
}
impl Default for BodyParameters {
    fn default() -> Self {
        Self {
            upkeep: 0.0005,
            max_acceleration: 100.0,
            max_angular_acceleration: 10.0,
            linear_drag: 1.0,
            angular_drag: 2.0,
            energy_scale: 1e-6,
            motor_efficiency: 0.25,
            force_loss: 0.01,
            torque_loss: 0.01,
            radius_scale: 10.0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlantConfig {
    pub initial_energy: f32,
    pub replacement_energy: f32,
    pub growth: f32,
    pub max_energy: f32,
    pub radius_scale: f32,
    /// Linear velocity decay rate (inverse physical time).
    pub linear_drag: f32,
    /// Angular velocity decay rate (inverse physical time).
    pub angular_drag: f32,
    pub initial_position: Placement,
    pub replacement_position: Placement,
    pub patches: super::patches::Config,
    /// Omit to retain the baseline's immediate fixed-count replacement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replenishment: Option<super::replenishment::Config>,
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
            linear_drag: parameters.linear_drag,
            angular_drag: parameters.angular_drag,
            initial_position: parameters.initial_position,
            replacement_position: parameters.replacement_position,
            patches: parameters.patches,
            replenishment: parameters.replenishment,
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
            linear_drag: self.linear_drag,
            angular_drag: self.angular_drag,
            initial_position: self.initial_position,
            replacement_position: self.replacement_position,
            patches: self.patches,
            replenishment: self.replenishment,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlantParameters {
    pub initial_energy: f32,
    pub replacement_energy: f32,
    pub growth: f32,
    pub max_energy: f32,
    pub radius_scale: f32,
    pub linear_drag: f32,
    pub angular_drag: f32,
    pub initial_position: Placement,
    pub replacement_position: Placement,
    pub patches: super::patches::Config,
    pub replenishment: Option<super::replenishment::Config>,
}
impl Default for PlantParameters {
    fn default() -> Self {
        Self {
            initial_energy: 1.0,
            replacement_energy: 0.0,
            growth: 0.0025,
            max_energy: 1.0,
            radius_scale: 10.0,
            linear_drag: 1.0,
            angular_drag: 2.0,
            initial_position: Placement::Random,
            replacement_position: Placement::Random,
            patches: super::patches::Config::default(),
            replenishment: None,
        }
    }
}

impl PlantParameters {
    pub fn max_radius(self) -> f32 {
        self.radius_scale * self.max_energy.sqrt()
    }
}
