use std::collections::{BTreeMap, VecDeque};

use glam::DVec2;
use rand::{RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::{AgentId, Environment, Population};

pub const HALF_SIZE: DVec2 = DVec2::new(630.0, 350.0);
const RIPE_ENERGY: f64 = 400.0;
const BIRTH_THRESHOLD: f64 = 400.0;
const START_ENERGY: f64 = 100.0;
const UPKEEP: f64 = 2.0;
const MAX_SPEED: f64 = 100.0;
const DT: f64 = 0.1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scenario {
    Debug,
    Baseline,
}

impl std::str::FromStr for Scenario {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "debug" => Ok(Self::Debug),
            "baseline" => Ok(Self::Baseline),
            _ => Err(format!("unknown scenario '{name}'")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Origin {
    Founder,
    Immigrant,
    Child { parent: AgentId },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FoodObservation {
    pub strength: f64,
    pub gradient: DVec2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveAction {
    pub direction: DVec2,
    pub speed_fraction: f64,
}

impl MoveAction {
    fn displacement(self) -> DVec2 {
        if !self.speed_fraction.is_finite() || self.direction.length() <= 0.01 {
            return DVec2::ZERO;
        }
        self.direction.normalize_or_zero() * (self.speed_fraction.clamp(0.0, 1.0) * MAX_SPEED * DT)
    }
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub position: DVec2,
    pub energy: f64,
}

impl Body {
    pub fn radius(&self) -> f64 {
        radius(self.energy)
    }
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
    Meal { id: AgentId, plant: u64, gain: f64 },
    Death { id: AgentId },
    Immigrant { id: AgentId },
    Birth { parent: AgentId, child: AgentId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub id: AgentId,
    pub observation: FoodObservation,
    pub action: MoveAction,
    pub energy: f64,
    pub position_before: DVec2,
    pub position_after: DVec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TickReport {
    pub tick: u64,
    pub events: Vec<Event>,
    pub decisions: Vec<Decision>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ForagingWorld {
    number: u64,
    rng: Xoshiro256PlusPlus,
    next_animal: u64,
    next_plant: u64,
    animals: BTreeMap<AgentId, Body>,
    plants: Vec<Plant>,
    plant_target: usize,
    immigrant_floor: usize,
}

impl ForagingWorld {
    pub fn new<P>(scenario: Scenario, world_seed: u64, population: &mut P) -> Self
    where
        P: Population<FoodObservation, MoveAction, Origin>,
    {
        let (plant_target, immigrant_floor) = match scenario {
            Scenario::Debug => (1, 0),
            Scenario::Baseline => (500, 20),
        };
        let mut world = Self {
            number: 0,
            rng: Xoshiro256PlusPlus::seed_from_u64(world_seed),
            next_animal: 1,
            next_plant: 1,
            plants: Vec::with_capacity(plant_target),
            animals: BTreeMap::new(),
            plant_target,
            immigrant_floor,
        };
        match scenario {
            Scenario::Debug => {
                world.spawn(population, Origin::Founder, DVec2::ZERO, 390.0);
                world.add_plant(DVec2::ZERO, RIPE_ENERGY);
            }
            Scenario::Baseline => {
                for _ in 0..immigrant_floor {
                    let position = world.random_position();
                    world.spawn(population, Origin::Founder, position, START_ENERGY);
                }
                for _ in 0..plant_target {
                    let position = world.random_position();
                    world.add_plant(position, RIPE_ENERGY);
                }
            }
        }
        world
    }

    pub fn number(&self) -> u64 {
        self.number
    }

    pub fn animals(&self) -> &BTreeMap<AgentId, Body> {
        &self.animals
    }

    pub fn plants(&self) -> &[Plant] {
        &self.plants
    }

    fn random_position(&mut self) -> DVec2 {
        DVec2::new(
            self.rng.random_range(-1.0..1.0),
            self.rng.random_range(-1.0..1.0),
        ) * HALF_SIZE
    }

    fn add_plant(&mut self, position: DVec2, energy: f64) {
        self.plants.push(Plant {
            id: self.next_plant,
            position,
            energy,
        });
        self.next_plant += 1;
    }

    fn spawn<P>(
        &mut self,
        population: &mut P,
        origin: Origin,
        position: DVec2,
        energy: f64,
    ) -> AgentId
    where
        P: Population<FoodObservation, MoveAction, Origin>,
    {
        let id = AgentId(self.next_animal);
        self.next_animal += 1;
        self.animals.insert(id, Body { position, energy });
        population.spawn(id, origin);
        id
    }

    fn feed_and_cull<P>(&mut self, population: &mut P, events: &mut Vec<Event>)
    where
        P: Population<FoodObservation, MoveAction, Origin>,
    {
        // Lower animal IDs get first claim. A meal increases the radius for the next contact.
        for (&id, body) in &mut self.animals {
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
        self.animals.retain(|&id, body| {
            if body.energy < 0.0 {
                population.despawn(id);
                events.push(Event::Death { id });
                false
            } else {
                true
            }
        });
    }

    fn replenish<P>(&mut self, population: &mut P, events: &mut Vec<Event>)
    where
        P: Population<FoodObservation, MoveAction, Origin>,
    {
        while self.plants.len() < self.plant_target {
            let position = self.random_position();
            self.add_plant(position, 0.0);
        }
        while self.animals.len() < self.immigrant_floor {
            let position = self.random_position();
            let id = self.spawn(population, Origin::Immigrant, position, START_ENERGY);
            events.push(Event::Immigrant { id });
        }
    }

    fn reproduce<P>(&mut self, population: &mut P, events: &mut Vec<Event>)
    where
        P: Population<FoodObservation, MoveAction, Origin>,
    {
        let mut parents: VecDeque<_> = self
            .animals
            .iter()
            .filter_map(|(&id, body)| (body.energy > BIRTH_THRESHOLD).then_some(id))
            .collect();
        while let Some(parent) = parents.pop_front() {
            let body = self.animals.get_mut(&parent).unwrap();
            body.energy *= 0.5;
            let energy = body.energy;
            let direction = DVec2::new(
                self.rng.random_range(-0.5..0.5),
                self.rng.random_range(-0.5..0.5),
            );
            let offset = 0.5 * body.radius() * direction.normalize_or(DVec2::X);
            let child_position = body.position + offset;
            body.position -= offset;
            let child = self.spawn(population, Origin::Child { parent }, child_position, energy);
            events.push(Event::Birth { parent, child });
            // The historical ordered traversal also visits newly appended children.
            if energy > BIRTH_THRESHOLD {
                parents.push_back(child);
            }
        }
    }

    fn observe(&self, position: DVec2) -> FoodObservation {
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

impl<P: Population<FoodObservation, MoveAction, Origin>> Environment<P> for ForagingWorld {
    type Report = TickReport;

    fn tick(&mut self, population: &mut P) -> TickReport {
        self.number += 1;
        let mut events = Vec::new();
        self.feed_and_cull(population, &mut events);
        self.replenish(population, &mut events);
        self.reproduce(population, &mut events);

        for body in self.animals.values_mut() {
            body.energy -= UPKEEP;
        }
        for plant in &mut self.plants {
            plant.energy = (plant.energy + 1.0).min(RIPE_ENERGY);
        }

        let mut decisions = Vec::with_capacity(self.animals.len());
        for (&id, body) in &self.animals {
            let observation = self.observe(body.position);
            let action = population.act(id, &observation);
            let position = clamp_position(body.position + action.displacement(), body.radius());
            decisions.push(Decision {
                id,
                observation,
                action,
                energy: body.energy,
                position_before: body.position,
                position_after: position,
            });
        }
        // Every controller above saw the same pre-movement state.
        for decision in &decisions {
            self.animals.get_mut(&decision.id).unwrap().position = decision.position_after;
        }
        for plant in &mut self.plants {
            plant.position = clamp_position(plant.position, plant.radius());
        }
        TickReport {
            tick: self.number,
            events,
            decisions,
        }
    }
}

#[cfg(test)]
mod tests;
