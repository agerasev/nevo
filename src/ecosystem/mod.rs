//! The foraging experiment: physical bodies and their minds share one lifetime.

mod config;
mod metrics;
mod update;

#[cfg(feature = "viewer")]
mod draw;

pub use config::{
    AnimalConfig, AnimalMetricsConfig, BodyConfig, BodyMetricsConfig, Config, MetricsConfig,
    Placement, PlantConfig, PlantMetricsConfig,
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

pub const HALF_SIZE: DVec2 = DVec2::new(630.0, 350.0);
const RIPE_ENERGY: f64 = 400.0;
const BIRTH_THRESHOLD: f64 = 400.0;
const START_ENERGY: f64 = 100.0;
const UPKEEP: f64 = 2.0;
const MAX_SPEED: f64 = 100.0;
const DT: f64 = 0.1;

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
    fn displacement(self) -> DVec2 {
        if self.direction.length() <= 0.01 {
            return DVec2::ZERO;
        }
        self.direction.normalize_or_zero() * (self.speed_fraction.clamp(0.0, 1.0) * MAX_SPEED * DT)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    pub id: u64,
    pub position: DVec2,
    pub energy: f64,
    #[serde(skip)]
    metrics: metrics::PlantCache,
}
impl Plant {
    pub fn new(id: u64, position: DVec2, energy: f64) -> Self {
        Self {
            id,
            position,
            energy,
            metrics: Default::default(),
        }
    }
    pub fn radius(&self) -> f64 {
        radius(self.energy)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub position: DVec2,
    pub energy: f64,
    #[serde(skip)]
    metrics: metrics::BodyCache,
}
impl Body {
    pub fn new(position: DVec2, energy: f64) -> Self {
        Self {
            position,
            energy,
            metrics: Default::default(),
        }
    }
    pub fn radius(&self) -> f64 {
        radius(self.energy)
    }
}

/// Body and mind are replaceable types, owned together and born or removed together.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animal<B, M> {
    pub body: B,
    pub mind: M,
}

fn radius(energy: f64) -> f64 {
    0.5 * energy.max(0.0).sqrt()
}
fn clamp_position(position: DVec2, radius: f64) -> DVec2 {
    let limit = (HALF_SIZE - radius).max(DVec2::ZERO);
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

/// Physical parameters for an arrival, whether at initialization or later.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Admission {
    /// None draws a uniform position using the world's spatial RNG.
    pub position: Option<DVec2>,
    pub energy: f64,
}
impl Default for Admission {
    fn default() -> Self {
        Self {
            position: None,
            energy: START_ENERGY,
        }
    }
}

fn random_position(rng: &mut Xoshiro256PlusPlus) -> DVec2 {
    DVec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * HALF_SIZE
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
) -> Result<()> {
    let next = next_plant
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("plant ID allocator exhausted"))?;
    plants.push(Plant::new(*next_plant, position, energy));
    *next_plant = next;
    Ok(())
}

fn validate_body(body: &Body) -> Result<()> {
    // Negative energy is legitimate between upkeep and the next cull.
    ensure!(body.energy.is_finite(), "non-finite animal energy");
    ensure!(
        body.position.is_finite() && body.position.abs().cmple(HALF_SIZE).all(),
        "invalid animal position"
    );
    Ok(())
}

fn validate_plants(plants: &[Plant], target: usize, next_plant: u64) -> Result<()> {
    ensure!(plants.len() == target, "plant count differs from target");
    let mut ids = BTreeSet::new();
    for plant in plants {
        ensure!(
            plant.id > 0 && plant.id < next_plant && ids.insert(plant.id),
            "invalid or duplicate plant ID"
        );
        ensure!(
            (0.0..=RIPE_ENERGY).contains(&plant.energy),
            "invalid plant energy"
        );
        ensure!(
            plant.position.is_finite() && plant.position.abs().cmple(HALF_SIZE).all(),
            "invalid plant position"
        );
    }
    Ok(())
}

/// Foraging with genetic minds whose inherited `AsRef<G>` genome stays unchanged
/// during inference; memory and other learned state belong to `Mind::Update`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecosystem<M = AnimalMind, G = RnnGenome, P = WeightMutation> {
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
    #[serde(skip)]
    metrics: metrics::Cache,
}

impl Ecosystem {
    pub fn new(config: Config, seed: u64) -> Result<Self> {
        Self::with_genetics(config, seed, None, WeightMutation::default())
    }
}

impl<M, G, P> Ecosystem<M, G, P> {
    pub fn empty(
        plant_target: usize,
        minimum_population: usize,
        world_seed: u64,
        genetic_seed: u64,
        immigrant_genome: Option<G>,
        mutation: P,
    ) -> Self {
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
            metrics: Default::default(),
        }
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

    /// Fill an empty plant layout with ripe food, after initial animal admission.
    pub fn seed_plants(&mut self, position: Option<DVec2>) -> Result<()> {
        ensure!(self.plants.is_empty(), "plants are already initialized");
        if let Some(position) = position {
            ensure!(
                position.is_finite() && position.abs().cmple(HALF_SIZE).all(),
                "invalid plant position"
            );
        }
        for _ in 0..self.plant_target {
            let position = position.unwrap_or_else(|| self.random_position());
            self.add_plant(position, RIPE_ENERGY)?;
        }
        Ok(())
    }

    fn random_position(&mut self) -> DVec2 {
        random_position(&mut self.rng)
    }

    fn add_plant(&mut self, position: DVec2, energy: f64) -> Result<()> {
        add_plant(&mut self.plants, &mut self.next_plant, position, energy)
    }

    fn sense(&self, position: DVec2) -> FoodObservation {
        let mut field = FoodObservation::default();
        for plant in &self.plants {
            let displacement = (plant.position - position) * (0.5 / HALF_SIZE.y);
            let distance = displacement.length() + 0.01;
            let mass = plant.energy / RIPE_ENERGY;
            field.strength += mass / distance;
            field.gradient += mass * displacement / (distance * distance * distance);
        }
        if !self.plants.is_empty() {
            field.strength /= self.plants.len() as f64;
            field.gradient /= self.plants.len() as f64;
        }
        field.gradient = if field.gradient.length() > 0.01 {
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
    P: Clone,
{
    pub fn with_genetics(
        config: Config<M::Config>,
        seed: u64,
        immigrant_genome: Option<G>,
        mutation: P,
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
            mutation,
        );
        world.admit(config.initial_animals, config.admission())?;
        world.seed_plants(config.initial_plant_position.position())?;
        world.validate()?;
        Ok(world)
    }

    /// Create complete animals through the same path at startup and during a run.
    pub fn admit(&mut self, count: usize, admission: Admission) -> Result<Vec<AnimalId>> {
        ensure!(admission.energy.is_finite(), "non-finite admission energy");
        if let Some(position) = admission.position {
            ensure!(
                position.is_finite() && position.abs().cmple(HALF_SIZE).all(),
                "invalid admission position"
            );
        }
        let mut arrivals = Vec::with_capacity(count);
        for _ in 0..count {
            let animal = Self::immigrant(
                self.immigrant_genome.as_ref(),
                admission,
                &mut self.rng,
                &mut self.context,
            )?;
            let id = allocate_animal(&mut self.next_animal)?;
            self.animals.insert(id, animal);
            arrivals.push(id);
        }
        Ok(arrivals)
    }

    fn immigrant(
        template: Option<&G>,
        admission: Admission,
        rng: &mut Xoshiro256PlusPlus,
        context: &mut GeneticContext,
    ) -> Result<Animal<Body, M>> {
        let position = admission.position.unwrap_or_else(|| random_position(rng));
        let genome = match template {
            Some(template) => template.clone(),
            None => G::random(context)?,
        };
        Ok(Animal {
            body: Body::new(position, admission.energy),
            mind: Self::develop(genome)?,
        })
    }

    fn develop(genome: G) -> Result<M> {
        let mind = M::try_from(genome).map_err(Into::into)?;
        mind.validate()?;
        Ok(mind)
    }
}

impl<M, G, P> Simulation for Ecosystem<M, G, P>
where
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction> + AsRef<G> + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    P: Clone,
{
    type Config = Config<M::Config>;
    type Report = TickReport;
    type Update = EcosystemUpdate<M, M::Update>;

    fn compute_update(
        &self,
        tick: u64,
        config: &Self::Config,
    ) -> Result<(TickReport, Self::Update)> {
        self.compute_tick(tick, config)
    }

    fn apply_update(&mut self, update: Self::Update) {
        update.apply(self);
    }

    fn validate(&self) -> Result<()> {
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
            validate_body(&animal.body)?;
            animal.mind.validate()?;
        }
        if let Some(template) = &self.immigrant_genome {
            Self::develop(template.clone())?;
        }
        validate_plants(&self.plants, self.plant_target, self.next_plant)
    }
}

#[cfg(test)]
mod tests;

impl PartialEq for Body {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position && self.energy == other.energy
    }
}
impl PartialEq for Plant {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.position == other.position && self.energy == other.energy
    }
}
impl<M: PartialEq, G: PartialEq, P: PartialEq> PartialEq for Ecosystem<M, G, P> {
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
    }
}
