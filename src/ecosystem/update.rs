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
}

enum PreparedMind<M, U> {
    Existing(U),
    New(Box<M>),
}

impl<M, U> EcosystemUpdate<M, U>
where
    M: Mind<GeneticContext, Update = U>,
{
    pub(super) fn apply<G, P>(self, world: &mut Ecosystem<M, G, P>) {
        world.animals.retain(|id, _| self.animals.contains_key(id));
        for (id, Animal { body, mind }) in self.animals {
            match mind {
                PreparedMind::Existing(update) => {
                    let animal = world.animals.get_mut(&id).expect("prepared animal exists");
                    animal.body = body;
                    animal.mind.apply(update);
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
    }
}

impl<M, G, P> Ecosystem<M, G, P>
where
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction> + AsRef<G> + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    P: Clone,
{
    pub(super) fn prepare_update(
        &self,
        tick: u64,
    ) -> Result<(EcosystemUpdate<M, M::Update>, TickReport)> {
        let mut update = EcosystemUpdate {
            rng: self.rng.clone(),
            context: self.context.clone(),
            next_animal: self.next_animal,
            next_plant: self.next_plant,
            animals: BTreeMap::new(),
            plants: self.plants.clone(),
        };
        let mut actions = Vec::with_capacity(self.animals.len());
        for (&id, animal) in &self.animals {
            if animal.body.energy < 0.0 {
                continue;
            }
            let observation = self.sense(animal.body.position);
            let (action, mind) = animal.mind.prepare(&mut update.context, observation)?;
            ensure!(
                action.direction.is_finite() && action.speed_fraction.is_finite(),
                "non-finite movement action"
            );
            actions.push((id, observation, action));
            update.animals.insert(
                id,
                Animal {
                    body: animal.body.clone(),
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
        // Lower IDs get first claim. A meal enlarges the next contact radius.
        for (&id, animal) in &mut update.animals {
            let body = &mut animal.body;
            update.plants.retain(|plant| {
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
        for (&id, animal) in &self.animals {
            if animal.body.energy < 0.0 {
                events.push(Event::Death { id });
            }
        }
        while update.plants.len() < self.plant_target {
            let position = random_position(&mut update.rng);
            add_plant(&mut update.plants, &mut update.next_plant, position, 0.0)?;
        }
        let count = self.minimum_population.saturating_sub(update.animals.len());
        for _ in 0..count {
            let Animal { body, mind } = Self::immigrant(
                self.immigrant_genome.as_ref(),
                Admission::default(),
                &mut update.rng,
                &mut update.context,
            )?;
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

        let mut parents: VecDeque<_> = update
            .animals
            .iter()
            .filter_map(|(&id, animal)| (animal.body.energy > BIRTH_THRESHOLD).then_some(id))
            .collect();
        while let Some(parent) = parents.pop_front() {
            let animal = update.animals.get_mut(&parent).expect("parent exists");
            let body = &mut animal.body;
            body.energy *= 0.5;
            let energy = body.energy;
            let direction = DVec2::new(
                update.rng.random_range(-0.5..0.5),
                update.rng.random_range(-0.5..0.5),
            );
            let offset = 0.5 * body.radius() * direction.normalize_or(DVec2::X);
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
            let mind = Self::develop(genome)?;
            let child = allocate_animal(&mut update.next_animal)?;
            update.animals.insert(
                child,
                Animal {
                    body: Body {
                        position: child_position,
                        energy,
                    },
                    mind: PreparedMind::New(Box::new(mind)),
                },
            );
            events.push(Event::Birth { parent, child });
            // Historical ordered traversal also visits newly appended children.
            if energy > BIRTH_THRESHOLD {
                parents.push_back(child);
            }
        }
        for (&id, animal) in &mut update.animals {
            let body = &mut animal.body;
            body.energy -= UPKEEP;
            body.position = clamp_position(body.position, body.radius());
            ensure!(id.0 > 0 && id.0 < update.next_animal, "invalid animal ID");
            validate_body(body)?;
        }
        for plant in &mut update.plants {
            plant.energy = (plant.energy + 1.0).min(RIPE_ENERGY);
            plant.position = clamp_position(plant.position, plant.radius());
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
        validate_plants(&update.plants, self.plant_target, update.next_plant)?;
        Ok((
            update,
            TickReport {
                tick,
                events,
                decisions,
            },
        ))
    }
}
