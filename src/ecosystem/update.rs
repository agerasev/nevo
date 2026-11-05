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
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction> + AsRef<G> + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    P: Clone + PartialEq + Default,
    G::Config: PartialEq,
{
    pub(super) fn compute_tick(
        &self,
        tick: u64,
    ) -> Result<(TickReport, EcosystemUpdate<M, M::Update>)> {
        let (metrics, reset_metrics) = self.metrics.begin(tick);
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
            if animal.body.energy < 0.0 {
                continue;
            }
            let observation = self.sense(animal.body.position);
            let (action, mind) = animal
                .mind
                .compute_update(observation, &mut update.context)?;
            ensure!(
                action.direction.is_finite() && action.speed_fraction.is_finite(),
                "non-finite movement action"
            );
            actions.push((id, observation, action));
            let mut body = animal.body.clone();
            if reset_metrics {
                body.metrics.reset();
            }
            update.animals.insert(
                id,
                Animal {
                    body,
                    mind: PreparedMind::Existing(mind),
                },
            );
        }
        let mut decisions = Vec::with_capacity(actions.len());
        for (id, observation, action) in actions {
            let body = &mut update
                .animals
                .get_mut(&id)
                .expect("acting animal exists")
                .body;
            let position = clamp_position(
                body.position + action.displacement(self.parameters.body),
                body.radius(),
                self.parameters.world,
            );
            decisions.push(Decision {
                id,
                observation,
                action,
                energy: body.energy,
                position_before: body.position,
                position_after: position,
            });
            let distance = if body.metrics.animal.distance.enabled() {
                body.position.distance(position)
            } else {
                0.0
            };
            body.metrics.observe_action(distance);
            body.position = position;
        }

        let mut events = Vec::new();
        // Lower IDs get first claim. A meal enlarges the next contact radius.
        for (&id, animal) in &mut update.animals {
            let body = &mut animal.body;
            update.plants.retain(|plant| {
                let distance = body.position.distance(plant.position);
                if distance
                    < self.parameters.feeding.contact_scale * (body.radius() + plant.radius())
                {
                    let gain = plant.energy * self.parameters.feeding.efficiency;
                    body.energy += gain;
                    if body.metrics.animal.food_gained.enabled() {
                        body.metrics.food_gained += gain;
                    }
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
        for (&id, animal) in &self.animals {
            if animal.body.energy < 0.0 {
                events.push(Event::Death { id });
                if update.metrics.observed_survival.enabled() {
                    update.metrics.observed_survival.push(if reset_metrics {
                        0.0
                    } else {
                        animal.body.metrics.active_ticks as f64
                    });
                }
            }
        }
        while update.plants.len() < self.plant_target {
            let position = self
                .parameters
                .plant
                .replacement_position
                .position()
                .unwrap_or_else(|| random_position(&mut update.rng, self.parameters.world));
            add_plant(
                &mut update.plants,
                &mut update.next_plant,
                position,
                self.parameters.plant.replacement_energy,
                self.parameters.plant.radius_scale,
            )?;
            update
                .plants
                .last_mut()
                .expect("new plant exists")
                .metrics
                .energy = self.metrics.plant_config.energy;
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

        // Pay for controlled travel before checking reproduction or splitting energy.
        // Sensing-time energy is also pre-movement/pre-feeding energy, so a meal
        // does not retroactively increase the fee. Arrivals have not acted;
        // birth offsets and growth clamping are not travel.
        if self.parameters.body.movement_cost > 0.0 {
            for decision in &decisions {
                let distance = decision.position_before.distance(decision.position_after);
                // Stationary animals pay no movement fee, even if E^p overflows.
                if distance == 0.0 {
                    continue;
                }
                let power = self.parameters.body.movement_energy_power;
                let mass = if power == 0.0 {
                    1.0 // Includes zero-energy animals: the flat fee is independent of E.
                } else {
                    decision.energy.powf(power)
                };
                let cost = mass * (distance * self.parameters.body.movement_cost);
                ensure!(cost.is_finite(), "non-finite movement energy cost");
                update
                    .animals
                    .get_mut(&decision.id)
                    .expect("acting animal exists")
                    .body
                    .energy -= cost;
            }
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
            if body.metrics.animal.offspring.enabled() {
                body.metrics.offspring += 1;
            }
            let before = body.energy;
            let energy = before * self.parameters.reproduction.child_energy_fraction;
            body.energy *= 1.0 - self.parameters.reproduction.child_energy_fraction;
            ensure!(
                energy < before && body.energy.is_finite(),
                "reproduction must reduce offspring energy"
            );
            let direction = DVec2::new(
                update.rng.random_range(-0.5..0.5),
                update.rng.random_range(-0.5..0.5),
            );
            let offset = self.parameters.reproduction.separation_scale
                * body.radius()
                * direction.normalize_or(DVec2::X);
            let child_position = body.position + offset;
            body.position -= offset;
            // In this ecosystem, inference changes mind state, not the inherited
            // genome exposed by AsRef<G>. Existing genomes stay borrowed.
            let inherited = match &animal.mind {
                PreparedMind::Existing(_) => self.animals[&parent].mind.as_ref(),
                PreparedMind::New(mind) => mind.as_ref().as_ref(),
            };
            let mut genome = inherited.clone();
            genome.mutate(&self.mutation, &mut update.context)?;
            let mind = Self::develop(
                genome,
                self.mind_config.as_ref(),
                self.metrics.config.enabled,
            )?;
            let child = allocate_animal(&mut update.next_animal)?;
            let mut body =
                Body::with_radius_scale(child_position, energy, self.parameters.body.radius_scale);
            body.metrics =
                metrics::BodyCache::new(self.metrics.body_config, self.metrics.animal_config);
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
        for (&id, animal) in &mut update.animals {
            let body = &mut animal.body;
            body.energy -= self.parameters.body.upkeep;
            body.position = clamp_position(body.position, body.radius(), self.parameters.world);
            ensure!(id.0 > 0 && id.0 < update.next_animal, "invalid animal ID");
            validate_body(body, self.parameters)?;
        }
        for plant in &mut update.plants {
            plant.energy =
                (plant.energy + self.parameters.plant.growth).min(self.parameters.plant.max_energy);
            plant.position = clamp_position(plant.position, plant.radius(), self.parameters.world);
        }
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
        };
        update.metrics.observe(&report);
        Ok((report, update))
    }
}
