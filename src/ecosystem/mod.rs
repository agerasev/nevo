//! The foraging experiment: physical bodies and their minds share one lifetime.

mod config;
mod metrics;
mod patches;
mod replenishment;
mod update;

#[cfg(feature = "viewer")]
mod draw;

pub use config::{
    AnimalConfig, BodyConfig, Config, FeedingConfig, GeneticsConfig, ImmigrationConfig,
    MetricsConfig, Placement, PlantConfig, ReproductionConfig, SensingConfig, WorldConfig,
};
pub use metrics::{Summary, Window};
pub use patches::Config as PatchConfig;
pub use replenishment::Config as ReplenishmentConfig;
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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    pub id: u64,
    pub position: DVec2,
    pub energy: f64,
    radius_scale: f64,
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
        }
    }
    pub fn radius(&self) -> f64 {
        radius(self.energy, self.radius_scale)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub position: DVec2,
    pub energy: f64,
    radius_scale: f64,
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

fn random_position<R: Rng + ?Sized>(rng: &mut R, world: WorldConfig) -> DVec2 {
    DVec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * world.half_size()
}

fn plant_position(
    placement: Placement,
    parameters: Parameters,
    rng: &mut Xoshiro256PlusPlus,
) -> Result<DVec2> {
    match placement.position() {
        Some(position) => Ok(position),
        None if parameters.plant.patches.count > 0 || parameters.plant.replenishment.is_some() => {
            let limit = patches::inset(parameters.world, parameters.plant.max_radius())?;
            Ok(patches::uniform_position(rng, limit))
        }
        None => Ok(random_position(rng, parameters.world)),
    }
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
    if parameters.plant.replenishment.is_none() {
        ensure!(plants.len() == target, "plant count differs from target");
    }
    let position_limit =
        if parameters.plant.patches.count > 0 || parameters.plant.replenishment.is_some() {
            parameters.world.half_size() - DVec2::splat(parameters.plant.max_radius())
        } else {
            parameters.world.half_size()
        };
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
            plant.position.is_finite() && plant.position.abs().cmple(position_limit).all(),
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
    patches: patches::Landscape,
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
    fn empty(
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
            patches: patches::Landscape::default(),
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
    /// Fill an empty plant layout using its configured initial admission settings.
    fn seed_plants(&mut self) -> Result<()> {
        ensure!(self.plants.is_empty(), "plants are already initialized");
        if let Some(config) = self.parameters.plant.replenishment
            && self.parameters.plant.initial_position == Placement::Random
        {
            let sampler = replenishment::Sampler::new(
                config,
                self.parameters.world,
                self.parameters.plant.max_radius(),
                self.parameters.plant.patches,
            )?;
            for _ in 0..config.max_initial_attempts {
                if self.plants.len() == self.plant_target {
                    break;
                }
                if let Some(position) = sampler.propose(&self.plants, &self.patches, &mut self.rng)
                {
                    self.add_plant(position, self.parameters.plant.initial_energy)?;
                }
            }
            ensure!(
                self.plants.len() == self.plant_target,
                "could only seed {} of {} plants within max_initial_attempts={}; lower ecosystem.plants, increase capacity, or raise the initial attempt budget",
                self.plants.len(),
                self.plant_target,
                config.max_initial_attempts
            );
            return Ok(());
        }
        for _ in 0..self.plant_target {
            let position = plant_position(
                self.parameters.plant.initial_position,
                self.parameters,
                &mut self.rng,
            )?;
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
        Ok(())
    }

    fn sense(&self, position: DVec2) -> FoodObservation {
        let mut field = FoodObservation::default();
        for plant in &self.plants {
            let displacement = (plant.position - position) * self.parameters.sensing.distance_scale;
            let distance = displacement.length() + self.parameters.sensing.softening;
            let mass = plant.energy / self.parameters.plant.max_energy;
            field.strength += mass / distance;
            field.gradient += mass * displacement / (distance * distance * distance);
        }
        field.strength *= self.parameters.sensing.signal_scale;
        field.gradient *= self.parameters.sensing.signal_scale;
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
        // Layout generation must not depend on animal initialization or consume
        // the spatial/genetic streams used by the running ecosystem.
        world.patches = patches::Landscape::generate(
            config.plant.patches,
            config.world,
            world.parameters.plant.max_radius(),
            &mut Xoshiro256PlusPlus::seed_from_u64(seeds.next_u64()),
        )?;
        world.initialization = config.genetics.initialization.clone();
        world.configure(&config)?;
        world.admit(config.initial_animals)?;
        world.seed_plants()?;
        world.validate()?;
        Ok(world)
    }

    /// All arrivals use the same persisted admission settings and construction path.
    fn admit(&mut self, count: usize) -> Result<Vec<AnimalId>> {
        let mut arrivals = Vec::with_capacity(count);
        for _ in 0..count {
            let animal = Self::make_immigrant(
                self.immigrant_genome.as_ref(),
                self.parameters,
                &self.initialization,
                &mut self.rng,
                &mut self.context,
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
            self.mind_config.as_ref(),
        )
    }

    fn make_immigrant(
        template: Option<&G>,
        parameters: Parameters,
        initialization: &G::Config,
        rng: &mut Xoshiro256PlusPlus,
        context: &mut GeneticContext,
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
        let body = Body::with_radius_scale(
            position,
            parameters.immigration.energy,
            parameters.body.radius_scale,
        );
        Ok(Animal {
            body,
            mind: Self::develop(genome, mind_config)?,
        })
    }

    fn develop(genome: G, config: Option<&M::Config>) -> Result<M> {
        let mut mind = M::try_from(genome).map_err(Into::into)?;
        match config {
            Some(config) => mind.configure(config)?,
            None => mind.configure(&M::Config::default())?,
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
            animal.mind.configure(&config.animal.mind)?;
        }
        self.metrics = metrics::Cache::new(config);
        self.mind_config = Some(config.animal.mind.clone());
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
        self.patches.validate(
            self.parameters.plant.patches,
            self.parameters.world,
            self.parameters.plant.max_radius(),
        )?;
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
            && self.patches == other.patches
            && self.plant_target == other.plant_target
            && self.minimum_population == other.minimum_population
            && self.immigrant_genome == other.immigrant_genome
            && self.mutation == other.mutation
            && self.initialization == other.initialization
            && self.parameters == other.parameters
    }
}
