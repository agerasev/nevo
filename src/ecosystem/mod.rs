//! The foraging experiment: physical bodies and their minds share one lifetime.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{Rng, RngExt, SeedableRng};
use rand_xoshiro::{SplitMix64, Xoshiro256PlusPlus};

use crate::{
    context::Cpu,
    mind::{
        Heredity, Mind,
        herbivore::{HerbivoreHeredity, HerbivoreMind},
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Debug,
    Baseline,
}

impl std::str::FromStr for Scenario {
    type Err = String;
    fn from_str(name: &str) -> std::result::Result<Self, Self::Err> {
        match name {
            "debug" => Ok(Self::Debug),
            "baseline" => Ok(Self::Baseline),
            _ => Err(format!("unknown scenario '{name}'")),
        }
    }
}

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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    pub id: u64,
    pub position: DVec2,
    pub energy: f64,
}
impl Plant {
    pub fn radius(&self) -> f64 {
        radius(self.energy)
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub position: DVec2,
    pub energy: f64,
}
impl Body {
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

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecosystem<M = HerbivoreMind, H = HerbivoreHeredity> {
    rng: Xoshiro256PlusPlus,
    context: GeneticContext,
    next_animal: u64,
    next_plant: u64,
    animals: BTreeMap<AnimalId, Animal<Body, M>>,
    plants: Vec<Plant>,
    plant_target: usize,
    minimum_population: usize,
    heredity: H,
}

impl Ecosystem {
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self> {
        Self::with_heredity(scenario, seed, HerbivoreHeredity::default())
    }
}

impl<M, H> Ecosystem<M, H> {
    pub fn empty(
        plant_target: usize,
        minimum_population: usize,
        world_seed: u64,
        genetic_seed: u64,
        heredity: H,
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
            heredity,
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
        DVec2::new(
            self.rng.random_range(-1.0..1.0),
            self.rng.random_range(-1.0..1.0),
        ) * HALF_SIZE
    }

    fn add_plant(&mut self, position: DVec2, energy: f64) -> Result<()> {
        let next = self
            .next_plant
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("plant ID allocator exhausted"))?;
        self.plants.push(Plant {
            id: self.next_plant,
            position,
            energy,
        });
        self.next_plant = next;
        Ok(())
    }

    fn spawn(&mut self, body: Body, mind: M) -> Result<AnimalId> {
        let next = self
            .next_animal
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("animal ID allocator exhausted"))?;
        let id = AnimalId(self.next_animal);
        self.animals.insert(id, Animal { body, mind });
        self.next_animal = next;
        Ok(id)
    }

    fn feed_and_cull(&mut self, events: &mut Vec<Event>) {
        // Lower IDs get first claim. A meal enlarges the next contact radius.
        for (&id, animal) in &mut self.animals {
            let body = &mut animal.body;
            if body.energy < 0.0 {
                continue;
            }
            self.plants.retain(|plant| {
                let distance = body.position.distance(plant.position);
                if distance < 0.8 * (body.radius() + plant.radius()) {
                    let gain = plant.energy * 0.1;
                    body.energy += gain;
                    events.push(Event::Meal {
                        id,
                        plant: plant.id,
                        gain,
                    });
                    false
                } else {
                    true
                }
            });
        }
        self.animals.retain(|&id, animal| {
            if animal.body.energy < 0.0 {
                events.push(Event::Death { id });
                false
            } else {
                true
            }
        });
    }

    fn replenish_plants(&mut self) -> Result<()> {
        while self.plants.len() < self.plant_target {
            let position = self.random_position();
            self.add_plant(position, 0.0)?;
        }
        Ok(())
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

impl<M, H> Ecosystem<M, H>
where
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction>,
    H: Heredity<GeneticContext, M>,
{
    pub fn with_heredity(scenario: Scenario, seed: u64, heredity: H) -> Result<Self> {
        let mut seeds = SplitMix64::seed_from_u64(seed);
        let world_seed = seeds.next_u64();
        let genetic_seed = seeds.next_u64();
        let (arrivals, plants, minimum, admission) = match scenario {
            Scenario::Debug => (
                1,
                1,
                0,
                Admission {
                    position: Some(DVec2::ZERO),
                    energy: 390.0,
                },
            ),
            Scenario::Baseline => (20, 500, 20, Admission::default()),
        };
        let mut world = Self::empty(plants, minimum, world_seed, genetic_seed, heredity);
        world.admit(arrivals, admission)?;
        world.seed_plants(admission.position)?;
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
            let position = admission.position.unwrap_or_else(|| self.random_position());
            let mind = self.heredity.immigrant(&mut self.context)?;
            arrivals.push(self.spawn(
                Body {
                    position,
                    energy: admission.energy,
                },
                mind,
            )?);
        }
        Ok(arrivals)
    }

    fn reproduce(&mut self, events: &mut Vec<Event>) -> Result<()> {
        let mut parents: VecDeque<_> = self
            .animals
            .iter()
            .filter_map(|(&id, animal)| (animal.body.energy > BIRTH_THRESHOLD).then_some(id))
            .collect();
        while let Some(parent) = parents.pop_front() {
            let animal = self.animals.get_mut(&parent).expect("parent exists");
            let body = &mut animal.body;
            body.energy *= 0.5;
            let energy = body.energy;
            let direction = DVec2::new(
                self.rng.random_range(-0.5..0.5),
                self.rng.random_range(-0.5..0.5),
            );
            let offset = 0.5 * body.radius() * direction.normalize_or(DVec2::X);
            let child_position = body.position + offset;
            body.position -= offset;
            let mind = self.heredity.offspring(&mut self.context, &animal.mind)?;
            let child = self.spawn(
                Body {
                    position: child_position,
                    energy,
                },
                mind,
            )?;
            events.push(Event::Birth { parent, child });
            // Historical ordered traversal also visits newly appended children.
            if energy > BIRTH_THRESHOLD {
                parents.push_back(child);
            }
        }
        Ok(())
    }
}

impl<M, H> Simulation for Ecosystem<M, H>
where
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction>,
    H: Heredity<GeneticContext, M>,
{
    type Report = TickReport;

    fn advance(&mut self, tick: u64) -> Result<TickReport> {
        let observations: Vec<_> = self
            .animals
            .iter()
            .filter(|(_, animal)| animal.body.energy >= 0.0)
            .map(|(&id, animal)| (id, self.sense(animal.body.position)))
            .collect();
        let mut actions = Vec::with_capacity(observations.len());
        for (id, observation) in observations {
            let action = self
                .animals
                .get_mut(&id)
                .expect("observed animal exists")
                .mind
                .process(&mut self.context, observation)?;
            ensure!(
                action.direction.is_finite() && action.speed_fraction.is_finite(),
                "non-finite movement action"
            );
            actions.push((id, observation, action));
        }
        let mut decisions = Vec::with_capacity(actions.len());
        for (id, observation, action) in actions {
            let body = &mut self
                .animals
                .get_mut(&id)
                .expect("acting animal exists")
                .body;
            let position = clamp_position(body.position + action.displacement(), body.radius());
            decisions.push(Decision {
                id,
                observation,
                action,
                energy: body.energy,
                position_before: body.position,
                position_after: position,
            });
            body.position = position;
        }
        let mut events = Vec::new();
        self.feed_and_cull(&mut events);
        self.replenish_plants()?;
        let count = self.minimum_population.saturating_sub(self.animals.len());
        for id in self.admit(count, Admission::default())? {
            events.push(Event::Immigrant { id });
        }
        self.reproduce(&mut events)?;
        for animal in self.animals.values_mut() {
            let body = &mut animal.body;
            body.energy -= UPKEEP;
            body.position = clamp_position(body.position, body.radius());
        }
        for plant in &mut self.plants {
            plant.energy = (plant.energy + 1.0).min(RIPE_ENERGY);
            plant.position = clamp_position(plant.position, plant.radius());
        }
        Ok(TickReport {
            tick,
            events,
            decisions,
        })
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
        ensure!(
            self.plants.len() == self.plant_target,
            "plant count differs from target"
        );
        for (id, animal) in &self.animals {
            ensure!(id.0 > 0 && id.0 < self.next_animal, "invalid animal ID");
            let body = &animal.body;
            // Negative energy is legitimate between upkeep and the next cull.
            ensure!(body.energy.is_finite(), "non-finite animal energy");
            ensure!(
                body.position.is_finite() && body.position.abs().cmple(HALF_SIZE).all(),
                "invalid animal position"
            );
            animal.mind.validate()?;
        }
        self.heredity.validate()?;
        let mut ids = BTreeSet::new();
        for plant in &self.plants {
            ensure!(
                plant.id > 0 && plant.id < self.next_plant && ids.insert(plant.id),
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
}

#[cfg(test)]
mod tests;
