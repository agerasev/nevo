use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::{
    actor::{
        Handle, HandleAllocator,
        population::{Lifecycle, PopulationAction, PopulationObservation},
    },
    environment::Environment,
};

pub const HALF_SIZE: DVec2 = DVec2::new(630.0, 350.0);
const RIPE_ENERGY: f64 = 400.0;
const BIRTH_THRESHOLD: f64 = 400.0;
const START_ENERGY: f64 = 100.0;
const UPKEEP: f64 = 2.0;
const MAX_SPEED: f64 = 100.0;
const DT: f64 = 0.1;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Origin {
    Immigrant,
    Child { parent: Handle },
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
        if !self.speed_fraction.is_finite() || self.direction.length() <= 0.01 {
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
pub struct Animal {
    pub position: DVec2,
    pub energy: f64,
}

impl Animal {
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
    Meal { id: Handle, plant: u64, gain: f64 },
    Death { id: Handle },
    Immigrant { id: Handle },
    Birth { parent: Handle, child: Handle },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub id: Handle,
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

/// The experiment decides whether to inject animals at this point in a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImmigrationRequest {
    pub population: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Immigration {
    pub count: usize,
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

pub type Observation = PopulationObservation<FoodObservation, Origin>;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ecosystem {
    rng: Xoshiro256PlusPlus,
    next_plant: u64,
    animals: BTreeMap<Handle, Animal>,
    plants: Vec<Plant>,
    plant_target: usize,
}

impl Ecosystem {
    pub fn empty(plant_target: usize, world_seed: u64) -> Self {
        Self {
            rng: Xoshiro256PlusPlus::seed_from_u64(world_seed),
            next_plant: 1,
            animals: BTreeMap::new(),
            plants: Vec::with_capacity(plant_target),
            plant_target,
        }
    }

    /// The experiment chooses how many arrivals to admit and their physical state.
    pub fn admit(
        &mut self,
        count: usize,
        admission: Admission,
        handles: &mut HandleAllocator,
    ) -> Result<Observation> {
        ensure!(admission.energy.is_finite(), "non-finite admission energy");
        if let Some(position) = admission.position {
            ensure!(
                position.is_finite() && position.abs().cmple(HALF_SIZE).all(),
                "invalid admission position"
            );
        }
        let mut changes = Vec::with_capacity(count);
        for _ in 0..count {
            let position = admission.position.unwrap_or_else(|| self.random_position());
            self.spawn(
                handles,
                Origin::Immigrant,
                position,
                admission.energy,
                &mut changes,
            )?;
        }
        Ok(self.lifecycle(changes))
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

    pub fn animals(&self) -> &BTreeMap<Handle, Animal> {
        &self.animals
    }

    pub fn plants(&self) -> &[Plant] {
        &self.plants
    }

    pub(crate) fn from_parts(
        rng: Xoshiro256PlusPlus,
        next_plant: u64,
        animals: BTreeMap<Handle, Animal>,
        plants: Vec<Plant>,
        plant_target: usize,
    ) -> Self {
        Self {
            rng,
            next_plant,
            animals,
            plants,
            plant_target,
        }
    }

    fn lifecycle(&self, changes: Vec<Lifecycle<Origin>>) -> Observation {
        PopulationObservation::Lifecycle {
            members: self.animals.keys().copied().collect(),
            changes,
        }
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

    fn spawn(
        &mut self,
        handles: &mut HandleAllocator,
        origin: Origin,
        position: DVec2,
        energy: f64,
        changes: &mut Vec<Lifecycle<Origin>>,
    ) -> Result<Handle> {
        let handle = handles.allocate()?;
        ensure!(
            !self.animals.contains_key(&handle),
            "duplicate animal handle"
        );
        self.animals.insert(handle, Animal { position, energy });
        changes.push(Lifecycle::Spawn { handle, origin });
        Ok(handle)
    }

    fn feed_and_cull(&mut self, events: &mut Vec<Event>, changes: &mut Vec<Lifecycle<Origin>>) {
        // Lower handles get first claim. A meal enlarges the next contact radius.
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
                changes.push(Lifecycle::Despawn { handle: id });
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

    fn immigrate(
        &mut self,
        count: usize,
        handles: &mut HandleAllocator,
        events: &mut Vec<Event>,
        changes: &mut Vec<Lifecycle<Origin>>,
    ) -> Result<()> {
        let PopulationObservation::Lifecycle {
            changes: arrivals, ..
        } = self.admit(count, Admission::default(), handles)?
        else {
            unreachable!("admission emits lifecycle observations")
        };
        for arrival in &arrivals {
            if let Lifecycle::Spawn { handle, .. } = arrival {
                events.push(Event::Immigrant { id: *handle });
            }
        }
        changes.extend(arrivals);
        Ok(())
    }

    fn reproduce(
        &mut self,
        handles: &mut HandleAllocator,
        events: &mut Vec<Event>,
        changes: &mut Vec<Lifecycle<Origin>>,
    ) -> Result<()> {
        let mut parents: VecDeque<_> = self
            .animals
            .iter()
            .filter_map(|(&id, body)| (body.energy > BIRTH_THRESHOLD).then_some(id))
            .collect();
        while let Some(parent) = parents.pop_front() {
            let body = self.animals.get_mut(&parent).expect("parent body exists");
            body.energy *= 0.5;
            let energy = body.energy;
            let direction = DVec2::new(
                self.rng.random_range(-0.5..0.5),
                self.rng.random_range(-0.5..0.5),
            );
            let offset = 0.5 * body.radius() * direction.normalize_or(DVec2::X);
            let child_position = body.position + offset;
            body.position -= offset;
            let child = self.spawn(
                handles,
                Origin::Child { parent },
                child_position,
                energy,
                changes,
            )?;
            events.push(Event::Birth { parent, child });
            // Historical ordered traversal also visits newly appended children.
            if energy > BIRTH_THRESHOLD {
                parents.push_back(child);
            }
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

impl Environment for Ecosystem {
    type Observation = Observation;
    type Action = PopulationAction<MoveAction>;
    type Report = TickReport;
    type Request = ImmigrationRequest;
    type Intervention = Immigration;

    fn observe(&self) -> Observation {
        PopulationObservation::Inputs {
            members: self.animals.keys().copied().collect(),
            inputs: self
                .animals
                .iter()
                .filter(|(_, body)| body.energy >= 0.0)
                .map(|(&id, body)| (id, self.sense(body.position)))
                .collect(),
        }
    }

    fn advance(
        &mut self,
        tick: u64,
        actions: Self::Action,
        handles: &mut HandleAllocator,
        mut intervene: impl FnMut(ImmigrationRequest) -> Result<Immigration>,
    ) -> Result<(TickReport, Observation)> {
        let eligible: BTreeSet<_> = self
            .animals
            .iter()
            .filter_map(|(&id, body)| (body.energy >= 0.0).then_some(id))
            .collect();
        let supplied: BTreeSet<_> = actions.iter().map(|(id, _)| *id).collect();
        ensure!(supplied.len() == actions.len(), "duplicate animal action");
        ensure!(
            supplied == eligible,
            "actions do not match eligible animals"
        );
        for (_, action) in &actions {
            ensure!(
                action.direction.is_finite() && action.speed_fraction.is_finite(),
                "non-finite movement action"
            );
        }
        let mut decisions = Vec::with_capacity(actions.len());
        for (id, action) in actions {
            // Only plants enter sensing; earlier movement cannot alter this value.
            let observation = self.sense(self.animals[&id].position);
            let body = self.animals.get_mut(&id).expect("validated animal action");
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
        let mut changes = Vec::new();
        self.feed_and_cull(&mut events, &mut changes);
        self.replenish_plants()?;
        let immigration = intervene(ImmigrationRequest {
            population: self.animals.len(),
        })?;
        self.immigrate(immigration.count, handles, &mut events, &mut changes)?;
        self.reproduce(handles, &mut events, &mut changes)?;

        for body in self.animals.values_mut() {
            body.energy -= UPKEEP;
            body.position = clamp_position(body.position, body.radius());
        }
        for plant in &mut self.plants {
            plant.energy = (plant.energy + 1.0).min(RIPE_ENERGY);
            plant.position = clamp_position(plant.position, plant.radius());
        }
        let report = TickReport {
            tick,
            events,
            decisions,
        };
        Ok((report, self.lifecycle(changes)))
    }

    fn validate(&self, handles: &HandleAllocator) -> Result<()> {
        ensure!(self.rng.state() != [0; 32], "world RNG state is all zero");
        ensure!(self.next_plant > 0, "invalid next plant ID");
        ensure!(
            self.plants.len() == self.plant_target,
            "plant count differs from target"
        );
        for (handle, body) in &self.animals {
            handles.validate_handle(*handle)?;
            // Negative energy is legitimate between upkeep and the next cull.
            ensure!(body.energy.is_finite(), "non-finite animal energy");
            ensure!(
                body.position.is_finite() && body.position.abs().cmple(HALF_SIZE).all(),
                "invalid animal position"
            );
        }
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
