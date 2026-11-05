//! The foraging experiment: physical bodies and their minds share one lifetime.

mod config;
mod metrics;
mod update;

#[cfg(feature = "viewer")]
mod draw;

pub use config::{
    AnimalConfig, AnimalMetricsConfig, BodyConfig, BodyMetricsConfig, Config, FeedingConfig,
    GeneticsConfig, ImmigrationConfig, MetricsConfig, Placement, PlantConfig, PlantMetricsConfig,
    ReproductionConfig, SensingConfig, WorldConfig,
};
pub use metrics::{
    AnimalSummary, BodySummary, EventSummary, PlantSummary, PopulationSummary, Summary, Window,
};
pub use update::EcosystemUpdate;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{Rng, RngExt, SeedableRng};
use rand_xoshiro::{SplitMix64, Xoshiro256PlusPlus};

use crate::{
    context::Cpu,
    genome::{Mutate, RandomInit},
    mind::{
        Mind,
        animal::AnimalMind,
        rnn::{RnnGenome, WeightMutation},
    },
    universe::Simulation,
};

use config::{BodyParameters, Parameters};

type GeneticContext = Cpu<Xoshiro256PlusPlus>;

/// Stable identity for lineage, event reports, and references within the experiment.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct AnimalId(u64);

impl AnimalId {
    pub fn serial(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FoodObservation {
    pub strength: f64,
    pub gradient: DVec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MoveAction {
    pub direction: DVec2,
    pub speed_fraction: f64,
}

impl MoveAction {
    fn displacement(self, body: BodyParameters) -> DVec2 {
        if self.direction.length() <= body.movement_dead_zone {
            return DVec2::ZERO;
        }
        self.direction.normalize_or_zero()
            * (self.speed_fraction.clamp(0.0, 1.0) * body.max_speed * body.movement_time_step)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    pub id: u64,
    pub position: DVec2,
    pub energy: f64,
    radius_scale: f64,
    #[serde(skip)]
    metrics: metrics::PlantCache,
}
impl Plant {
    pub fn new(id: u64, position: DVec2, energy: f64) -> Self {
        Self::with_radius_scale(id, position, energy, PlantConfig::default().radius_scale)
    }
    fn with_radius_scale(id: u64, position: DVec2, energy: f64, radius_scale: f64) -> Self {
        Self {
            id,
            position,
            energy,
            radius_scale,
            metrics: Default::default(),
        }
    }
    pub fn radius(&self) -> f64 {
        radius(self.energy, self.radius_scale)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub position: DVec2,
    pub energy: f64,
    radius_scale: f64,
    #[serde(skip)]
    metrics: metrics::BodyCache,
}
impl Body {
    pub fn new(position: DVec2, energy: f64) -> Self {
        Self::with_radius_scale(position, energy, BodyConfig::default().radius_scale)
    }
    fn with_radius_scale(position: DVec2, energy: f64, radius_scale: f64) -> Self {
        Self {
            position,
            energy,
            radius_scale,
            metrics: Default::default(),
        }
    }
    pub fn radius(&self) -> f64 {
        radius(self.energy, self.radius_scale)
    }
}

/// Body and mind are replaceable types, owned together and born or removed together.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animal<B, M> {
    pub body: B,
    pub mind: M,
}

fn radius(energy: f64, scale: f64) -> f64 {
    scale * energy.max(0.0).sqrt()
}
fn clamp_position(position: DVec2, radius: f64, world: WorldConfig) -> DVec2 {
    let limit = (world.half_size() - radius).max(DVec2::ZERO);
    position.clamp(-limit, limit)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Meal { id: AnimalId, plant: u64, gain: f64 },
    Death { id: AnimalId },
    Immigrant { id: AnimalId },
    Birth { parent: AnimalId, child: AnimalId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub id: AnimalId,
    pub observation: FoodObservation,
    pub action: MoveAction,
    /// Energy when sensed, before feeding, splitting, and upkeep.
    pub energy: f64,
    pub position_before: DVec2,
    /// Movement endpoint, before birth offsets and clamping after growth.
    pub position_after: DVec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickReport {
    pub tick: u64,
    pub events: Vec<Event>,
    pub decisions: Vec<Decision>,
}

fn random_position(rng: &mut Xoshiro256PlusPlus, world: WorldConfig) -> DVec2 {
    DVec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * world.half_size()
}

fn allocate_animal(next_animal: &mut u64) -> Result<AnimalId> {
    let next = next_animal
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("animal ID allocator exhausted"))?;
    let id = AnimalId(*next_animal);
    *next_animal = next;
    Ok(id)
}

fn add_plant(
    plants: &mut Vec<Plant>,
    next_plant: &mut u64,
    position: DVec2,
    energy: f64,
    radius_scale: f64,
) -> Result<()> {
    let next = next_plant
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("plant ID allocator exhausted"))?;
    plants.push(Plant::with_radius_scale(
        *next_plant,
        position,
        energy,
        radius_scale,
    ));
    *next_plant = next;
    Ok(())
}

fn validate_body(body: &Body, parameters: Parameters) -> Result<()> {
    // Negative energy is legitimate between upkeep and the next cull.
    ensure!(body.energy.is_finite(), "non-finite animal energy");
    ensure!(
        body.position.is_finite()
            && body
                .position
                .abs()
                .cmple(parameters.world.half_size())
                .all(),
        "invalid animal position"
    );
    ensure!(
        body.radius_scale == parameters.body.radius_scale && body.radius().is_finite(),
        "invalid animal radius"
    );
    Ok(())
}

fn validate_plants(
    plants: &[Plant],
    target: usize,
    next_plant: u64,
    parameters: Parameters,
) -> Result<()> {
    ensure!(plants.len() == target, "plant count differs from target");
    let mut ids = BTreeSet::new();
    for plant in plants {
        ensure!(
            plant.id > 0 && plant.id < next_plant && ids.insert(plant.id),
            "invalid or duplicate plant ID"
        );
        ensure!(
            (0.0..=parameters.plant.max_energy).contains(&plant.energy),
            "invalid plant energy"
        );
        ensure!(
            plant.position.is_finite()
                && plant
                    .position
                    .abs()
                    .cmple(parameters.world.half_size())
                    .all(),
            "invalid plant position"
        );
        ensure!(
            plant.radius_scale == parameters.plant.radius_scale && plant.radius().is_finite(),
            "invalid plant radius"
        );
    }
    Ok(())
}

/// Foraging with genetic minds whose inherited `AsRef<G>` genome stays unchanged
/// during inference; memory and other learned state belong to `Mind::Update`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecosystem<
    M = AnimalMind,
    G = RnnGenome,
    P = WeightMutation,
    MC = <M as Mind<GeneticContext>>::Config,
    IC = <G as RandomInit<GeneticContext>>::Config,
> {
    rng: Xoshiro256PlusPlus,
    context: GeneticContext,
    next_animal: u64,
    next_plant: u64,
    animals: BTreeMap<AnimalId, Animal<Body, M>>,
    plants: Vec<Plant>,
    plant_target: usize,
    minimum_population: usize,
    immigrant_genome: Option<G>,
    mutation: P,
    initialization: IC,
    parameters: Parameters,
    #[serde(skip)]
    metrics: metrics::Cache,
    #[serde(skip)]
    mind_config: Option<MC>,
}

impl Ecosystem {
    pub fn new(config: Config, seed: u64) -> Result<Self> {
        config.genetics.initialization.validate()?;
        config.genetics.mutation.validate()?;
        Self::with_genetics(config, seed, None)
    }
}

impl<M, G, P, MC, IC> Ecosystem<M, G, P, MC, IC> {
    pub fn empty(
        plant_target: usize,
        minimum_population: usize,
        world_seed: u64,
        genetic_seed: u64,
        immigrant_genome: Option<G>,
        mutation: P,
    ) -> Self
    where
        IC: Default,
    {
        Self {
            rng: Xoshiro256PlusPlus::seed_from_u64(world_seed),
            context: Cpu {
                rng: Xoshiro256PlusPlus::seed_from_u64(genetic_seed),
            },
            next_animal: 1,
            next_plant: 1,
            animals: BTreeMap::new(),
            plants: Vec::with_capacity(plant_target),
            plant_target,
            minimum_population,
            immigrant_genome,
            mutation,
            initialization: IC::default(),
            parameters: Parameters::default(),
            metrics: Default::default(),
            mind_config: None,
        }
    }

    pub fn world_config(&self) -> WorldConfig {
        self.parameters.world
    }

    pub fn animals(&self) -> &BTreeMap<AnimalId, Animal<Body, M>> {
        &self.animals
    }
    pub fn plants(&self) -> &[Plant] {
        &self.plants
    }
    pub fn context(&self) -> &GeneticContext {
        &self.context
    }
    pub fn minimum_population(&self) -> usize {
        self.minimum_population
    }
    pub fn set_minimum_population(&mut self, minimum: usize) {
        self.minimum_population = minimum;
    }

    /// Fill an empty plant layout using its configured initial admission settings.
    pub fn seed_plants(&mut self) -> Result<()> {
        ensure!(self.plants.is_empty(), "plants are already initialized");
        for _ in 0..self.plant_target {
            let position = self
                .parameters
                .plant
                .initial_position
                .position()
                .unwrap_or_else(|| random_position(&mut self.rng, self.parameters.world));
            self.add_plant(position, self.parameters.plant.initial_energy)?;
        }
        Ok(())
    }

    fn add_plant(&mut self, position: DVec2, energy: f64) -> Result<()> {
        add_plant(
            &mut self.plants,
            &mut self.next_plant,
            position,
            energy,
            self.parameters.plant.radius_scale,
        )?;
        self.plants
            .last_mut()
            .expect("new plant exists")
            .metrics
            .energy = self.metrics.plant_config.energy;
        Ok(())
    }

    fn sense(&self, position: DVec2) -> FoodObservation {
        let mut field = FoodObservation::default();
        for plant in &self.plants {
            let displacement = (plant.position - position)
                * (self.parameters.sensing.distance_scale / self.parameters.world.half_size().y);
            let distance = displacement.length() + self.parameters.sensing.softening;
            let mass = plant.energy / self.parameters.plant.max_energy;
            field.strength += mass / distance;
            field.gradient += mass * displacement / (distance * distance * distance);
        }
        if !self.plants.is_empty() {
            field.strength /= self.plants.len() as f64;
            field.gradient /= self.plants.len() as f64;
        }
        field.gradient = if field.gradient.length() > self.parameters.sensing.gradient_dead_zone {
            field.gradient.normalize()
        } else {
            DVec2::ZERO
        };
        field
    }
}

impl<M, G, P> Ecosystem<M, G, P>
where
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction> + AsRef<G> + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    P: Clone + PartialEq + Default,
    G::Config: PartialEq,
{
    pub fn with_genetics(
        config: Config<M::Config, G::Config, P>,
        seed: u64,
        immigrant_genome: Option<G>,
    ) -> Result<Self> {
        config.validate()?;
        let mut seeds = SplitMix64::seed_from_u64(seed);
        let world_seed = seeds.next_u64();
        let genetic_seed = seeds.next_u64();
        let mut world = Self::empty(
            config.plants,
            config.minimum_population,
            world_seed,
            genetic_seed,
            immigrant_genome,
            config.genetics.mutation.clone(),
        );
        world.parameters = config.parameters();
        world.initialization = config.genetics.initialization.clone();
        world.configure(&config)?;
        world.admit(config.initial_animals)?;
        world.seed_plants()?;
        world.validate()?;
        Ok(world)
    }

    /// All arrivals use the same persisted admission settings and construction path.
    pub fn admit(&mut self, count: usize) -> Result<Vec<AnimalId>> {
        let mut arrivals = Vec::with_capacity(count);
        for _ in 0..count {
            let animal = Self::make_immigrant(
                self.immigrant_genome.as_ref(),
                self.parameters,
                &self.initialization,
                &mut self.rng,
                &mut self.context,
                &self.metrics,
                self.mind_config.as_ref(),
            )?;
            let id = allocate_animal(&mut self.next_animal)?;
            self.animals.insert(id, animal);
            arrivals.push(id);
        }
        Ok(arrivals)
    }

    fn immigrant(
        &self,
        rng: &mut Xoshiro256PlusPlus,
        context: &mut GeneticContext,
    ) -> Result<Animal<Body, M>> {
        Self::make_immigrant(
            self.immigrant_genome.as_ref(),
            self.parameters,
            &self.initialization,
            rng,
            context,
            &self.metrics,
            self.mind_config.as_ref(),
        )
    }

    fn make_immigrant(
        template: Option<&G>,
        parameters: Parameters,
        initialization: &G::Config,
        rng: &mut Xoshiro256PlusPlus,
        context: &mut GeneticContext,
        metrics: &metrics::Cache,
        mind_config: Option<&M::Config>,
    ) -> Result<Animal<Body, M>> {
        let position = parameters
            .immigration
            .position
            .position()
            .unwrap_or_else(|| random_position(rng, parameters.world));
        let genome = match template {
            Some(template) => template.clone(),
            None => G::random(initialization, context)?,
        };
        let mut body = Body::with_radius_scale(
            position,
            parameters.immigration.energy,
            parameters.body.radius_scale,
        );
        body.metrics = metrics::BodyCache::new(metrics.body_config, metrics.animal_config);
        Ok(Animal {
            body,
            mind: Self::develop(genome, mind_config, metrics.config.enabled)?,
        })
    }

    fn develop(genome: G, config: Option<&M::Config>, metrics_enabled: bool) -> Result<M> {
        let mut mind = M::try_from(genome).map_err(Into::into)?;
        match config {
            Some(config) => mind.configure(config, metrics_enabled)?,
            None => mind.configure(&M::Config::default(), false)?,
        }
        mind.validate()?;
        Ok(mind)
    }
}

impl<M, G, P> Simulation for Ecosystem<M, G, P>
where
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction> + AsRef<G> + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    P: Clone + PartialEq + Default,
    G::Config: PartialEq,
{
    type Config = Config<M::Config, G::Config, P>;
    type Report = TickReport;
    type Update = EcosystemUpdate<M, M::Update>;

    fn configure(&mut self, config: &Self::Config) -> Result<()> {
        config.validate()?;
        ensure!(
            self.parameters == config.parameters()
                && self.initialization == config.genetics.initialization
                && self.mutation == config.genetics.mutation
                && self.plant_target == config.plants
                && self.minimum_population == config.minimum_population,
            "model parameters differ from the stored ecosystem"
        );
        for animal in self.animals.values_mut() {
            animal
                .mind
                .configure(&config.animal.mind, config.metrics.enabled)?;
        }
        self.metrics = metrics::Cache::new(config);
        self.mind_config = Some(config.animal.mind.clone());
        for animal in self.animals.values_mut() {
            animal.body.metrics =
                metrics::BodyCache::new(self.metrics.body_config, self.metrics.animal_config);
        }
        for plant in &mut self.plants {
            plant.metrics.energy = self.metrics.plant_config.energy;
        }
        Ok(())
    }

    fn compute_update(&self, tick: u64) -> Result<(TickReport, Self::Update)> {
        self.compute_tick(tick)
    }

    fn apply_update(&mut self, update: Self::Update) {
        update.apply(self);
    }

    fn validate(&self) -> Result<()> {
        self.parameters.validate()?;
        ensure!(self.rng.state() != [0; 32], "world RNG state is all zero");
        ensure!(
            self.context.rng.state() != [0; 32],
            "genetic RNG state is all zero"
        );
        ensure!(
            self.next_animal > 0 && self.next_plant > 0,
            "invalid next ID"
        );
        for (id, animal) in &self.animals {
            ensure!(id.0 > 0 && id.0 < self.next_animal, "invalid animal ID");
            validate_body(&animal.body, self.parameters)?;
            animal.mind.validate()?;
        }
        if let Some(template) = &self.immigrant_genome {
            // Validation also runs before runtime settings are restored.
            let mind = M::try_from(template.clone()).map_err(Into::into)?;
            mind.validate()?;
        }
        validate_plants(
            &self.plants,
            self.plant_target,
            self.next_plant,
            self.parameters,
        )
    }
}

#[cfg(test)]
mod tests;

impl PartialEq for Body {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position
            && self.energy == other.energy
            && self.radius_scale == other.radius_scale
    }
}
impl PartialEq for Plant {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.position == other.position
            && self.energy == other.energy
            && self.radius_scale == other.radius_scale
    }
}
impl<M: PartialEq, G: PartialEq, P: PartialEq, MC, IC: PartialEq> PartialEq
    for Ecosystem<M, G, P, MC, IC>
{
    fn eq(&self, other: &Self) -> bool {
        self.rng == other.rng
            && self.context == other.context
            && self.next_animal == other.next_animal
            && self.next_plant == other.next_plant
            && self.animals == other.animals
            && self.plants == other.plants
            && self.plant_target == other.plant_target
            && self.minimum_population == other.minimum_population
            && self.immigrant_genome == other.immigrant_genome
            && self.mutation == other.mutation
            && self.initialization == other.initialization
            && self.parameters == other.parameters
    }
}
