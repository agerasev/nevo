use std::collections::VecDeque;

use super::*;

/// Owned changes for one tick. Existing animals carry only their next body and
/// mind update; only immigrants and offspring carry a complete new mind.
pub struct EcosystemUpdate<M, U> {
    rng: Xoshiro256PlusPlus,
    context: GeneticContext,
    next_animal: u64,
    next_plant: u64,
    animals: BTreeMap<AnimalId, Animal<Body, PreparedMind<M, U>>>,
    plants: Vec<Plant>,
    metrics: metrics::Cache,
}

enum PreparedMind<M, U> {
    Existing(U),
    New(Box<M>),
}

impl<M, U> EcosystemUpdate<M, U>
where
    M: Mind<GeneticContext, Update = U>,
{
    pub(super) fn apply<G, P, IC>(self, world: &mut Ecosystem<M, G, P, M::Config, IC>) {
        world.animals.retain(|id, _| self.animals.contains_key(id));
        for (id, Animal { body, mind }) in self.animals {
            match mind {
                PreparedMind::Existing(update) => {
                    let animal = world.animals.get_mut(&id).expect("prepared animal exists");
                    animal.body = body;
                    animal.mind.apply_update(update);
                }
                PreparedMind::New(mind) => {
                    world.animals.insert(id, Animal { body, mind: *mind });
                }
            }
        }
        world.rng = self.rng;
        world.context = self.context;
        world.next_animal = self.next_animal;
        world.next_plant = self.next_plant;
        world.plants = self.plants;
        world.metrics = self.metrics;
    }
}

impl<M, G, P> Ecosystem<M, G, P>
where
    M: Mind<GeneticContext, Input = AnimalObservation, Output = MotorAction>
        + BrainStructure
        + AsRef<G>
        + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    P: Clone + PartialEq + Default,
    G::Config: PartialEq,
{
    pub(super) fn compute_tick(
        &self,
        tick: u64,
    ) -> Result<(TickReport, EcosystemUpdate<M, M::Update>)> {
        let metrics = self.metrics.begin(tick);
        let mut update = EcosystemUpdate {
            rng: self.rng.clone(),
            context: self.context.clone(),
            next_animal: self.next_animal,
            next_plant: self.next_plant,
            animals: BTreeMap::new(),
            plants: self.plants.clone(),
            metrics,
        };
        let mut actions = Vec::with_capacity(self.animals.len());
        for (&id, animal) in &self.animals {
            if animal.body.energy <= 0.0 {
                continue;
            }
            let observation = self.sense(id);
            let (action, mind) = animal
                .mind
                .compute_update(observation, &mut update.context)?;
            ensure!(
                action.thrust.is_finite() && action.turn.is_finite(),
                "non-finite movement action"
            );
            let action = MotorAction {
                thrust: action.thrust.clamp(0.0, 1.0),
                turn: action.turn.clamp(-1.0, 1.0),
            };
            actions.push((id, observation, action));
            let body = animal.body.clone();
            update.animals.insert(
                id,
                Animal {
                    body,
                    mind: PreparedMind::Existing(mind),
                },
            );
        }
        let mut driven: Vec<_> = update
            .animals
            .values_mut()
            .zip(&actions)
            .map(|(animal, (_, _, action))| (&mut animal.body, *action))
            .collect();
        let physical = physics::advance(
            &mut driven,
            &mut update.plants,
            &self.obstacles,
            self.parameters,
        )?;
        let mut decisions = Vec::with_capacity(actions.len());
        for (&(id, observation, action), motion) in actions.iter().zip(&physical.motions) {
            let before = &self.animals[&id].body;
            let body = &update.animals[&id].body;
            let parameters = self.parameters.body;
            let movement_energy = if parameters.energy_scale == 0.0 {
                0.0
            } else {
                (parameters.energy_scale / parameters.motor_efficiency) * motion.mechanical_work
                    + (parameters.energy_scale * parameters.force_loss) * motion.force_squared_time
                    + (parameters.energy_scale * parameters.torque_loss)
                        * motion.torque_squared_time
            };
            ensure!(
                movement_energy.is_finite() && movement_energy >= 0.0,
                "non-finite motor energy cost"
            );
            decisions.push(Decision {
                id,
                observation,
                action,
                energy: body.energy,
                position_before: before.position,
                position_after: body.position,
                heading_before: before.heading,
                heading_after: body.heading,
                distance: motion.distance,
                movement_energy,
            });
        }

        let mut events = Vec::new();
        // Lower IDs get first claim. Edible contacts merge only the assimilated mass.
        let mut eaten = BTreeSet::new();
        for (animal_index, (&id, animal)) in update.animals.iter_mut().enumerate() {
            let body = &mut animal.body;
            for (plant_index, plant) in update.plants.iter().enumerate() {
                if eaten.contains(&plant.id) {
                    continue;
                }
                let distance = body.position.distance(plant.position);
                if physical.contacts.contains(&(animal_index, plant_index))
                    || distance <= body.radius() + plant.radius()
                {
                    let gain = feeding::assimilate(
                        body,
                        plant,
                        self.parameters.feeding.efficiency,
                        self.parameters.physics.mass_per_energy,
                    )?;
                    events.push(Event::Meal {
                        id,
                        plant: plant.id,
                        gain,
                    });
                    eaten.insert(plant.id);
                }
            }
        }
        update.plants.retain(|plant| !eaten.contains(&plant.id));
        for (&id, animal) in &self.animals {
            if animal.body.energy <= 0.0 {
                events.push(Event::Death { id });
            }
        }
        if let Some(config) = self.parameters.plant.replenishment {
            let sampler = replenishment::Sampler::new(
                config,
                self.parameters.world,
                self.parameters.plant.max_radius(),
                self.parameters.plant.patches,
            )?;
            let attempts = sampler.attempts(&mut update.rng);
            let before = update.plants.len();
            for _ in 0..attempts {
                if let Some(position) =
                    sampler.propose(&update.plants, &self.patches, &mut update.rng)
                    && obstacle::clear(
                        position,
                        self.parameters.plant.max_radius(),
                        &self.obstacles,
                    )
                {
                    add_plant(
                        &mut update.plants,
                        &mut update.next_plant,
                        position,
                        self.parameters.plant.replacement_energy,
                        self.parameters.plant.radius_scale,
                    )?;
                }
            }
            update
                .metrics
                .observe_replenishment(attempts, update.plants.len() - before);
        } else {
            while update.plants.len() < self.plant_target {
                let position = plant_position(
                    self.parameters.plant.replacement_position,
                    self.parameters,
                    &self.obstacles,
                    &mut update.rng,
                )?;
                add_plant(
                    &mut update.plants,
                    &mut update.next_plant,
                    position,
                    self.parameters.plant.replacement_energy,
                    self.parameters.plant.radius_scale,
                )?;
            }
        }
        let count = self.minimum_population.saturating_sub(update.animals.len());
        for _ in 0..count {
            let Animal { body, mind } = self.immigrant(&mut update.rng, &mut update.context)?;
            let id = allocate_animal(&mut update.next_animal)?;
            update.animals.insert(
                id,
                Animal {
                    body,
                    mind: PreparedMind::New(Box::new(mind)),
                },
            );
            events.push(Event::Immigrant { id });
        }

        // Pay integrated work/losses once, before reproduction. Coasting,
        // birth offsets and growth clamping do not create a propulsion charge.
        for decision in &decisions {
            update
                .animals
                .get_mut(&decision.id)
                .expect("acting animal exists")
                .body
                .energy -= decision.movement_energy;
        }

        let mut parents: VecDeque<_> = update
            .animals
            .iter()
            .filter_map(|(&id, animal)| {
                (animal.body.energy > self.parameters.reproduction.threshold).then_some(id)
            })
            .collect();
        while let Some(parent) = parents.pop_front() {
            let animal = update.animals.get_mut(&parent).expect("parent exists");
            let body = &mut animal.body;
            let before = body.energy;
            let energy = before * self.parameters.reproduction.child_energy_fraction;
            body.energy *= 1.0 - self.parameters.reproduction.child_energy_fraction;
            ensure!(
                energy < before && body.energy.is_finite(),
                "reproduction must reduce offspring energy"
            );
            let direction = Vec2::from_angle(
                body.heading + update.rng.random_range(0.0..std::f32::consts::TAU),
            );
            let offset = self.parameters.reproduction.separation_scale * body.radius() * direction;
            let child_position = body.position + offset;
            body.position -= offset;
            let heading = body.heading;
            let velocity = body.velocity;
            let angular_velocity = body.angular_velocity;
            // In this ecosystem, inference changes mind state, not the inherited
            // genome exposed by AsRef<G>. Existing genomes stay borrowed.
            let inherited = match &animal.mind {
                PreparedMind::Existing(_) => self.animals[&parent].mind.as_ref(),
                PreparedMind::New(mind) => mind.as_ref().as_ref(),
            };
            let mut genome = inherited.clone();
            genome.mutate(&self.mutation, &mut update.context)?;
            let mind = Self::develop(genome, self.mind_config.as_ref())?;
            let child = allocate_animal(&mut update.next_animal)?;
            let mut body =
                Body::with_radius_scale(child_position, energy, self.parameters.body.radius_scale);
            // Splitting preserves motion; each mind chooses its own next action.
            body.heading = heading;
            body.velocity = velocity;
            body.angular_velocity = angular_velocity;
            update.animals.insert(
                child,
                Animal {
                    body,
                    mind: PreparedMind::New(Box::new(mind)),
                },
            );
            events.push(Event::Birth { parent, child });
            // Historical ordered traversal also visits newly appended children.
            if energy > self.parameters.reproduction.threshold {
                parents.push_back(child);
            }
        }
        let mut brain_upkeep_energy = 0.0_f64;
        for (&id, animal) in &mut update.animals {
            let mind = match &animal.mind {
                PreparedMind::Existing(_) => &self.animals[&id].mind,
                PreparedMind::New(mind) => mind.as_ref(),
            };
            let brain_cost = self.parameters.brain_weight_upkeep
                * mind.brain_size().map_or(0, |size| size.weights) as f32;
            ensure!(brain_cost.is_finite(), "non-finite brain upkeep");
            brain_upkeep_energy += f64::from(brain_cost);
            let body = &mut animal.body;
            body.energy -= self.parameters.body.upkeep + brain_cost;
            body.confine(self.parameters.world);
            ensure!(id.0 > 0 && id.0 < update.next_animal, "invalid animal ID");
            validate_body(body, self.parameters)?;
        }
        let mut plant_growth = 0.0;
        for plant in &mut update.plants {
            let growth = self.parameters.plant.growth
                * self
                    .patches
                    .growth_multiplier(plant.position, self.parameters.plant.patches);
            let energy = (plant.energy + growth).min(self.parameters.plant.max_energy);
            plant_growth += energy - plant.energy;
            // New biomass is at rest in the environmental frame.
            if energy > 0.0 {
                let fraction = plant.energy / energy;
                plant.velocity *= fraction;
                plant.angular_velocity *= fraction * fraction;
            }
            plant.energy = energy;
            plant.position = clamp_position(plant.position, plant.radius(), self.parameters.world);
            let limit = (self.parameters.world.half_size() - plant.radius()).max(Vec2::ZERO);
            for axis in 0..2 {
                if (plant.position[axis] >= limit[axis] && plant.velocity[axis] > 0.0)
                    || (plant.position[axis] <= -limit[axis] && plant.velocity[axis] < 0.0)
                {
                    plant.velocity[axis] = 0.0;
                }
            }
        }
        update.metrics.observe_plant_growth(plant_growth);
        ensure!(update.rng.state() != [0; 32], "world RNG state is all zero");
        ensure!(
            update.context.rng.state() != [0; 32],
            "genetic RNG state is all zero"
        );
        ensure!(
            update.next_animal > 0 && update.next_plant > 0,
            "invalid next ID"
        );
        validate_plants(
            &update.plants,
            self.plant_target,
            update.next_plant,
            self.parameters,
        )?;
        let report = TickReport {
            tick,
            events,
            decisions,
            brain_upkeep_energy,
        };
        update.metrics.observe(&report);
        Ok((report, update))
    }
}
