use super::*;
use crate::genome::{Genome, Mutate, RandomInit};

// Explicit fixture economics, independent of the current experiment defaults.
const HALF_SIZE: Vec2 = Vec2::new(630.0, 350.0);
const START_ENERGY: f32 = 0.25;
const RIPE_ENERGY: f32 = 1.0;
const UPKEEP: f32 = 0.005;

#[derive(Clone, Copy)]
struct Admission {
    position: Option<Vec2>,
    energy: f32,
}
impl Default for Admission {
    fn default() -> Self {
        Self {
            position: None,
            energy: START_ENERGY,
        }
    }
}
// Fixtures may inject particular bodies without changing the world's ongoing policy.
impl<M, G, P> Ecosystem<M, G, P>
where
    M: Mind<GeneticContext, Input = AnimalObservation, Output = MotorAction>
        + BrainStructure
        + AsRef<G>
        + TryFrom<G>,
    M::Error: Into<anyhow::Error>,
    G: RandomInit<GeneticContext> + Mutate<GeneticContext, P>,
    G::Config: PartialEq,
    P: Clone + PartialEq + Default,
{
    fn admit_with(&mut self, count: usize, admission: Admission) -> Result<Vec<AnimalId>> {
        let previous = self.parameters.immigration;
        self.parameters.immigration = ImmigrationConfig {
            energy: admission.energy,
            position: admission
                .position
                .map(Placement::Fixed)
                .unwrap_or(Placement::Random),
        };
        let result = self.admit(count);
        self.parameters.immigration = previous;
        if let Ok(ids) = &result {
            for id in ids {
                self.animals.get_mut(id).unwrap().body.heading = 0.0;
            }
        }
        result
    }
    fn seed_plants_at(&mut self, position: Option<Vec2>) -> Result<()> {
        let previous = self.parameters.plant.initial_position;
        self.parameters.plant.initial_position =
            position.map(Placement::Fixed).unwrap_or(Placement::Random);
        let result = self.seed_plants();
        self.parameters.plant.initial_position = previous;
        result
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
struct FixedGenome<const FAIL_INIT: bool = false> {
    action: MotorAction,
    allele: u64,
    fail_mind: bool,
    reject_mind: bool,
}

impl<const FAIL_INIT: bool> Genome<GeneticContext> for FixedGenome<FAIL_INIT> {}

impl<const FAIL_INIT: bool> RandomInit<GeneticContext> for FixedGenome<FAIL_INIT> {
    type Config = ();
    fn random(_: &(), cx: &mut GeneticContext) -> Result<Self> {
        let allele = cx.rng.next_u64();
        ensure!(!FAIL_INIT, "initialization failed");
        Ok(Self {
            allele,
            ..Self::default()
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize)]
struct FixedMutation {
    increment: u64,
    fail: bool,
    reject_child: bool,
}

impl<const FAIL_INIT: bool> Mutate<GeneticContext, FixedMutation> for FixedGenome<FAIL_INIT> {
    fn mutate(&mut self, mutation: &FixedMutation, cx: &mut GeneticContext) -> Result<()> {
        if mutation.fail {
            // A backend may fail after consuming randomness or changing scratch state.
            self.allele = cx.rng.next_u64();
            anyhow::bail!("mutation failed");
        }
        self.allele = self.allele.wrapping_add(mutation.increment);
        self.reject_mind = mutation.reject_child;
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, serde::Serialize)]
struct FixedMind<const FAIL_INIT: bool = false> {
    genome: FixedGenome<FAIL_INIT>,
    calls: usize,
}

impl<const FAIL_INIT: bool> BrainStructure for FixedMind<FAIL_INIT> {}

impl<const FAIL_INIT: bool> AsRef<FixedGenome<FAIL_INIT>> for FixedMind<FAIL_INIT> {
    fn as_ref(&self) -> &FixedGenome<FAIL_INIT> {
        &self.genome
    }
}

impl<const FAIL_INIT: bool> TryFrom<FixedGenome<FAIL_INIT>> for FixedMind<FAIL_INIT> {
    type Error = anyhow::Error;

    fn try_from(genome: FixedGenome<FAIL_INIT>) -> Result<Self> {
        ensure!(!genome.reject_mind, "mind construction failed");
        Ok(Self { genome, calls: 0 })
    }
}

impl<const FAIL_INIT: bool> Mind<GeneticContext> for FixedMind<FAIL_INIT> {
    type Input = AnimalObservation;
    type Output = MotorAction;
    type Update = usize;
    type Config = ();

    fn compute_update(
        &self,
        _: AnimalObservation,
        cx: &mut GeneticContext,
    ) -> Result<(MotorAction, usize)> {
        if self.genome.fail_mind {
            // Even a failed inference that consumes scratch randomness cannot publish it.
            cx.rng.next_u64();
        }
        ensure!(!self.genome.fail_mind, "mind failed");
        Ok((self.genome.action, self.calls + 1))
    }

    fn apply_update(&mut self, calls: usize) {
        self.calls = calls;
    }
}

struct Fixture {
    world: Ecosystem<FixedMind, FixedGenome, FixedMutation>,
    number: u64,
}

impl Fixture {
    fn debug(seed: u64) -> Self {
        let mut world = Ecosystem::empty(1, 0, seed, seed, None, FixedMutation::default());
        world.parameters.body.upkeep = UPKEEP;
        world.parameters.body.energy_scale = 0.0;
        world
            .admit_with(
                1,
                Admission {
                    position: Some(Vec2::ZERO),
                    energy: 0.975,
                },
            )
            .unwrap();
        let contact = world.animals[&id(1)].body.radius() + world.parameters.plant.max_radius();
        world.seed_plants_at(Some(Vec2::new(contact, 0.0))).unwrap();
        Self { world, number: 0 }
    }

    fn tick(&mut self) -> TickReport {
        self.number += 1;
        self.world.advance(self.number).unwrap()
    }

    fn action(&mut self, action: MotorAction) {
        for animal in self.world.animals.values_mut() {
            animal.mind.genome.action = action;
        }
    }

    fn sense(&self) -> AnimalObservation {
        self.world.sense(id(1))
    }
}

fn encode(value: &impl serde::Serialize) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).unwrap();
    bytes
}

fn id(serial: u64) -> AnimalId {
    AnimalId(serial)
}

#[test]
fn first_claimant_gets_the_only_meal_and_replacement_starts_small() {
    let mut fixture = Fixture::debug(1);
    let first = id(1);
    let body = &mut fixture.world.animals.get_mut(&first).unwrap().body;
    body.energy = START_ENERGY;
    body.position = Vec2::new(-15.0, 0.0);
    fixture.world.plants[0].position = Vec2::ZERO;
    let second = fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(Vec2::new(15.0, 0.0)),
                energy: START_ENERGY,
            },
        )
        .unwrap()[0];

    let tick = fixture.tick();
    assert_eq!(
        tick.events,
        [Event::Meal {
            id: first,
            plant: 1,
            gain: 0.1
        }]
    );
    assert_eq!(fixture.world.animals[&first].body.energy, 0.345);
    assert_eq!(fixture.world.animals[&second].body.energy, 0.245);
    assert_eq!(fixture.world.plants[0].energy, 0.0025);
    assert_ne!(fixture.world.plants[0].id, 1);
}

#[test]
fn upkeep_defers_death_and_replacement_first_acts_on_the_following_tick() {
    let mut fixture = Fixture::debug(2);
    fixture.world.minimum_population = 1;
    fixture.world.plants[0].position = Vec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = UPKEEP;

    let starving = fixture.tick();
    assert!(starving.events.is_empty());
    assert_eq!(starving.decisions[0].energy, UPKEEP);
    assert_eq!(fixture.world.animals[&id(1)].body.energy, 0.0);
    assert!(starving.decisions[0].position_after.is_finite());

    fixture.world.plants[0].position = starving.decisions[0].position_after;
    let replacement = fixture.tick();
    assert_eq!(
        replacement.events,
        [Event::Death { id: id(1) }, Event::Immigrant { id: id(2) }]
    );
    assert_eq!(fixture.world.animals.len(), 1);
    assert!(!fixture.world.animals.contains_key(&id(1)));
    assert_eq!(fixture.world.animals[&id(2)].mind.calls, 0);
    assert!(replacement.decisions.is_empty());
    let next = fixture.tick();
    assert_eq!(next.decisions.len(), 1);
    assert_eq!(
        (next.decisions[0].id, next.decisions[0].energy),
        (id(2), 0.245)
    );
    assert_eq!(fixture.world.animals[&id(2)].mind.calls, 1);
}

#[test]
fn field_uses_maturity_distance_and_body_axes() {
    let mut fixture = Fixture::debug(3);
    fixture.world.plants[0].position = Vec2::new(70.0, 0.0);
    fixture
        .world
        .add_plant(Vec2::new(-70.0, 0.0), RIPE_ENERGY)
        .unwrap();
    let balanced = fixture.sense();
    assert_eq!(balanced.gradient, Vec2::ZERO);
    assert!((balanced.strength - 0.004 / 0.11).abs() < 2.0 * f32::EPSILON * balanced.strength);
    fixture.world.plants[1].energy *= 0.5;
    let unbalanced = fixture.sense();
    assert!((unbalanced.gradient - Vec2::X).length() < 1e-12);
    assert!((unbalanced.strength - 0.003 / 0.11).abs() < 2.0 * f32::EPSILON * unbalanced.strength);
}

#[test]
fn sensing_is_independent_of_arena_dimensions() {
    let mut fixture = Fixture::debug(3);
    fixture.world.plants[0].position = Vec2::new(70.0, 35.0);
    let sensed = fixture.sense();
    fixture.world.parameters.world.width *= 2.0;
    fixture.world.parameters.world.height *= 4.0;
    assert_eq!(fixture.sense(), sensed);
}

#[test]
fn zero_energy_plants_do_not_dilute_sensing() {
    let mut fixture = Fixture::debug(3);
    fixture.world.plants[0].position = Vec2::new(70.0, 35.0);
    let sensed = fixture.sense();
    fixture.world.add_plant(Vec2::new(-70.0, 0.0), 0.0).unwrap();
    assert_eq!(fixture.sense(), sensed);
}

#[test]
fn landscapes_are_seeded_independently_of_population_and_brain_size() {
    let mut config = Config {
        initial_animals: 0,
        plants: 0,
        minimum_population: 0,
        ..Config::default()
    };
    config.plant.patches.count = 3;
    config.obstacles = ObstacleConfig::Random(RandomObstacles {
        count: 8,
        radius_min: 12.0,
        radius_max: 24.0,
    });
    let empty = Ecosystem::new(config.clone(), 17).unwrap();
    config.initial_animals = 8;
    config.plants = 20;
    config.genetics.initialization.hidden_units = 16;
    let populated = Ecosystem::new(config.clone(), 17).unwrap();
    assert_eq!(empty.patches, populated.patches);
    assert_eq!(empty.obstacles, populated.obstacles);
    let other_seed = Ecosystem::new(config.clone(), 18).unwrap();
    assert_ne!(empty.patches, other_seed.patches);
    assert_ne!(empty.obstacles, other_seed.obstacles);
    config.plant.patches.count = 5;
    assert_eq!(
        empty.obstacles,
        Ecosystem::new(config.clone(), 17).unwrap().obstacles
    );
    config.plant.patches.count = 3;
    config.obstacles = ObstacleConfig::default();
    assert_eq!(empty.patches, Ecosystem::new(config, 17).unwrap().patches);
}

#[test]
fn plant_admission_reserves_mature_clearance_but_moving_plants_use_their_current_radius() {
    for replenishment in [None, Some(ReplenishmentConfig::default())] {
        let mut config = Config {
            initial_animals: 0,
            plants: 1,
            minimum_population: 0,
            ..Config::default()
        };
        config.plant.patches.count = usize::from(replenishment.is_none());
        config.plant.replenishment = replenishment;
        config.plant.initial_energy = 0.25;
        let edge = config.world.half_size();
        config.plant.initial_position = Placement::Fixed(edge);
        assert!(config.validate().is_err());
        config.plant.initial_position = Placement::Random;
        let mut world = Ecosystem::new(config, 3).unwrap();
        let radius = world.plants[0].radius();
        world.plants[0].position = Vec2::new(edge.x - radius, 0.0);
        world.validate().unwrap();
        world.plants[0].position.x = edge.x + 1.0;
        assert!(world.validate().is_err());
    }
}

#[test]
fn initial_replenishment_reports_exhaustion_when_capacity_cannot_fit_the_count() {
    let mut config = Config {
        initial_animals: 0,
        plants: 2,
        minimum_population: 0,
        world: WorldConfig {
            width: 40.0,
            height: 40.0,
        },
        ..Config::default()
    };
    // The first plant is accepted, then its broad crowding kernel exceeds the
    // low capacity throughout this small arena.
    config.plant.replenishment = Some(ReplenishmentConfig {
        density_floor: 0.0001,
        density_peak: 0.0001,
        recovery_rate: 0.0,
        max_initial_attempts: 3,
        ..ReplenishmentConfig::default()
    });
    config.validate().unwrap();
    let error = Ecosystem::new(config, 3).unwrap_err().to_string();
    assert!(error.contains("could only seed 1 of 2 plants"), "{error}");
    assert!(error.contains("max_initial_attempts=3"), "{error}");
}

#[test]
fn obstacles_bound_random_admission_attempts_for_animals_and_plants() {
    for (animals, plants, density_based) in [(1, 0, false), (0, 1, false), (0, 1, true)] {
        let mut config = Config {
            initial_animals: animals,
            plants,
            minimum_population: 0,
            world: WorldConfig {
                width: 100.0,
                height: 100.0,
            },
            ..Config::default()
        };
        // The circles fit the arena but their union leaves no room for a radius-10 body.
        config.obstacles = ObstacleConfig::Explicit(
            [(-20.0, -20.0), (-20.0, 20.0), (20.0, -20.0), (20.0, 20.0)]
                .into_iter()
                .map(|(x, y)| Obstacle {
                    position: Vec2::new(x, y),
                    radius: 30.0,
                })
                .collect(),
        );
        config.animal.body.radius_scale = 20.0;
        config.physics.max_placement_attempts = 3;
        if density_based {
            config.plant.replenishment = Some(ReplenishmentConfig {
                max_initial_attempts: 3,
                ..ReplenishmentConfig::default()
            });
        }
        config.validate().unwrap();
        let error = Ecosystem::new(config, 3).unwrap_err().to_string();
        let budget = if density_based {
            "max_initial_attempts=3"
        } else {
            "within 3 attempts"
        };
        assert!(error.contains(budget), "{error}");
    }
}

#[test]
fn admission_rejects_bodies_larger_than_the_arena() {
    for (initial_animals, plants) in [(1, 0), (0, 1)] {
        let config = Config {
            initial_animals,
            plants,
            minimum_population: 0,
            world: WorldConfig {
                width: 5.0,
                height: 5.0,
            },
            ..Config::default()
        };
        assert!(
            Ecosystem::new(config, 3)
                .unwrap_err()
                .to_string()
                .contains("fit inside the arena")
        );
    }
}

#[test]
fn checkpoint_rejects_changed_obstacle_geometry() {
    use crate::universe::Universe;
    let mut config = Config {
        initial_animals: 0,
        plants: 0,
        minimum_population: 0,
        obstacles: ObstacleConfig::Explicit(vec![Obstacle {
            position: Vec2::ZERO,
            radius: 20.0,
        }]),
        ..Config::default()
    };
    let universe = Universe::new(config.clone(), 3).unwrap();
    let ObstacleConfig::Explicit(obstacles) = &mut config.obstacles else {
        unreachable!()
    };
    obstacles[0].radius = 21.0;
    assert!(universe.restore(config).is_err());
}

#[test]
fn zero_recovery_leaves_harvested_plants_absent_instead_of_topping_up() {
    let mut config = Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        ..Config::default()
    };
    config.immigration.position = Placement::Fixed(Vec2::ZERO);
    config.plant.initial_position = Placement::Fixed(Vec2::new(15.0, 0.0));
    config.plant.replenishment = Some(ReplenishmentConfig {
        recovery_rate: 0.0,
        ..ReplenishmentConfig::default()
    });
    let mut world: Ecosystem =
        Ecosystem::with_genetics(config, 3, Some(RnnGenome::default())).unwrap();
    let report = world.advance(1).unwrap();
    assert!(
        report
            .events
            .iter()
            .any(|event| matches!(event, Event::Meal { .. }))
    );
    assert!(world.plants.is_empty());
    world.advance(2).unwrap();
    assert!(world.plants.is_empty());
    world.validate().unwrap();
}

#[test]
fn replenishment_can_grow_an_empty_population_within_its_tick_attempt_budget() {
    use crate::metrics::Metrics;

    let mut config = Config {
        initial_animals: 0,
        plants: 0,
        minimum_population: 0,
        world: WorldConfig {
            width: 100.0,
            height: 100.0,
        },
        ..Config::default()
    };
    config.plant.replenishment = Some(ReplenishmentConfig {
        density_floor: 0.000625,
        density_peak: 0.000625,
        recovery_rate: 1.0,
        max_attempts_per_tick: 4,
        ..ReplenishmentConfig::default()
    });
    let mut world = Ecosystem::new(config, 3).unwrap();
    assert!(world.plants.is_empty());
    assert_eq!(world.metrics().plant_recruitment_attempts_per_tick, None);
    assert_eq!(world.metrics().plant_recruits_per_tick, None);
    world.advance(1).unwrap();
    assert!((1..=4).contains(&world.plants.len()));
    let summary = world.metrics();
    assert_eq!(summary.plant_count, Some(world.plants.len() as u64));
    assert_eq!(summary.plant_recruitment_attempts_per_tick, Some(4.0));
    assert_eq!(
        summary.plant_recruits_per_tick,
        Some(world.plants.len() as f64)
    );
    world.validate().unwrap();
}

#[test]
fn newly_seeded_plants_grow_once_before_their_first_physical_motion() {
    let mut config = Config {
        initial_animals: 0,
        plants: 8,
        minimum_population: 0,
        ..Config::default()
    };
    config.plant.initial_energy = 0.0;
    config.plant.growth = 1.0;
    config.plant.replenishment = Some(ReplenishmentConfig {
        recovery_rate: 0.0,
        ..ReplenishmentConfig::default()
    });
    let mut world = Ecosystem::new(config, 3).unwrap();
    let positions: Vec<_> = world.plants.iter().map(|plant| plant.position).collect();
    world.advance(1).unwrap();
    assert!(world.plants.iter().all(|plant| plant.energy == 1.0));
    assert_eq!(
        world
            .plants
            .iter()
            .map(|plant| plant.position)
            .collect::<Vec<_>>(),
        positions
    );
}

#[test]
fn capped_growth_and_replacement_energy_balance_the_plant_stock() {
    use crate::metrics::Metrics;

    let mut config = Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        ..Config::default()
    };
    config.immigration.position = Placement::Fixed(Vec2::ZERO);
    config.plant.initial_position = Placement::Fixed(Vec2::new(15.0, 0.0));
    config.plant.replacement_position = Placement::Fixed(Vec2::new(100.0, 0.0));
    config.plant.replacement_energy = 0.9;
    config.plant.growth = 0.1;
    config.plant.patches = PatchConfig {
        count: 2,
        growth_floor: 2.0,
        growth_peak: 2.0,
        ..PatchConfig::default()
    };
    let mut world: Ecosystem =
        Ecosystem::with_genetics(config.clone(), 7, Some(RnnGenome::default())).unwrap();
    assert_eq!(world.plants[0].position, Vec2::new(15.0, 0.0));
    assert_eq!(world.metrics().plant_growth_per_tick, None);
    let stock_before = world.plants[0].energy;
    let report = world.advance(1).unwrap();
    assert!(matches!(report.events[0], Event::Meal { gain: 0.1, .. }));
    assert_eq!(world.plants[0].position, Vec2::new(100.0, 0.0));
    let growth = world.metrics().plant_growth_per_tick.unwrap();
    assert!(
        (growth - 0.1).abs() < f64::from(f32::EPSILON),
        "only growth below the cap counts"
    );
    assert_eq!(
        f64::from(stock_before) - 1.0 + f64::from(config.plant.replacement_energy) + growth,
        f64::from(world.plants[0].energy)
    );
}

#[test]
fn separated_resting_plants_grow_faster_near_patches() {
    let mut config = Config {
        initial_animals: 0,
        plants: 2,
        minimum_population: 0,
        ..Config::default()
    };
    config.plant.initial_energy = 0.0;
    config.plant.growth = 0.1;
    config.plant.patches = PatchConfig {
        count: 1,
        spread: 10.0,
        growth_floor: 0.1,
        growth_peak: 2.0,
    };
    let mut world = Ecosystem::new(config, 3).unwrap();
    world.patches = serde_json::from_str(r#"{"centers": [[0.0, 0.0]]}"#).unwrap();
    world.plants[0].position = Vec2::ZERO;
    world.plants[1].position = Vec2::new(620.0, 0.0);
    let positions: Vec<_> = world.plants.iter().map(|plant| plant.position).collect();
    world.advance(1).unwrap();
    assert!(world.plants[0].energy > world.plants[1].energy);
    for tick in 2..=150 {
        world.advance(tick).unwrap();
    }
    assert!(world.plants.iter().all(|p| p.energy == 1.0));
    assert_eq!(
        world.plants.iter().map(|p| p.position).collect::<Vec<_>>(),
        positions
    );
}

#[test]
fn body_stops_at_the_edge_and_cannot_eat_across_it() {
    let mut fixture = Fixture::debug(4);
    fixture.action(MotorAction {
        thrust: 1.0,
        turn: 0.0,
    });
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.position.x = HALF_SIZE.x - 5.0;
    body.energy = START_ENERGY;
    fixture.world.plants[0].position.x = -HALF_SIZE.x + fixture.world.plants[0].radius();
    let tick = fixture.tick();
    assert!(tick.events.is_empty());
    let body = &fixture.world.animals[&id(1)].body;
    assert!(body.position.x + body.radius() <= HALF_SIZE.x);
    assert!(body.position.x > 0.0);
}

#[test]
fn sensing_observes_food_before_it_is_eaten() {
    let mut fixture = Fixture::debug(7);
    let sensed = fixture.sense();
    let report = fixture.tick();
    assert_eq!(report.decisions[0].observation, sensed);
    assert_eq!(report.decisions[0].energy, 0.975);
    assert_eq!(fixture.world.plants[0].energy, 0.0025);
}

#[test]
fn movement_enters_contact_before_feeding_and_birth() {
    let mut fixture = Fixture::debug(8);
    fixture.world.parameters.body.max_acceleration = 20.0;
    fixture.world.parameters.body.linear_drag = 0.0;
    fixture.world.parameters.physics.time_step = 0.5;
    fixture.action(MotorAction {
        thrust: 1.0,
        turn: 0.0,
    });
    fixture.world.plants[0].position = Vec2::new(20.0, 0.0);
    let body = &fixture.world.animals[&id(1)].body;
    assert!(
        body.position.distance(fixture.world.plants[0].position)
            > body.radius() + fixture.world.plants[0].radius()
    );
    let report = fixture.tick();
    assert_eq!(report.decisions.len(), 1);
    assert_eq!(report.decisions[0].position_before, Vec2::ZERO);
    assert!(report.decisions[0].position_after.x > 0.0);
    assert_eq!(
        report.events,
        [
            Event::Meal {
                id: id(1),
                plant: 1,
                gain: 0.1
            },
            Event::Birth {
                parent: id(1),
                child: id(2)
            },
        ]
    );
    assert!(
        fixture
            .world
            .animals
            .values()
            .all(|animal| animal.body.position.is_finite())
    );
}

#[test]
fn feeding_growth_cannot_leave_a_body_outside_the_arena() {
    let mut fixture = Fixture::debug(9);
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.energy = START_ENERGY;
    body.position = Vec2::new(HALF_SIZE.x - body.radius(), 0.0);
    // Put food against the inner side of the animal, with both disks inside the wall.
    fixture.world.plants[0].position = body.position - Vec2::new(body.radius() + 10.0, 0.0);
    fixture.tick();
    let body = &fixture.world.animals[&id(1)].body;
    assert_eq!(body.energy, 0.345);
    assert!(body.position.x + body.radius() <= HALF_SIZE.x);
}

#[test]
fn replacements_follow_culling_and_precede_births() {
    let mut fixture = Fixture::debug(10);
    fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(Vec2::ZERO),
                energy: -0.0025,
            },
        )
        .unwrap();
    fixture.world.minimum_population = 2;
    let report = fixture.tick();
    assert!(matches!(report.events[1], Event::Death { id: animal } if animal == id(2)));
    assert!(matches!(report.events[2], Event::Immigrant { id: animal } if animal == id(3)));
    assert!(matches!(report.events[3], Event::Birth { child, .. } if child == id(4)));
    assert_eq!(
        fixture.world.animals[&id(3)].body.energy,
        START_ENERGY - UPKEEP
    );
    assert_eq!(fixture.world.animals[&id(3)].mind.calls, 0);
    assert_eq!(fixture.world.animals[&id(4)].mind.calls, 0);
}

#[test]
fn failures_in_processing_mutation_or_construction_leave_the_published_universe_unchanged() {
    use crate::universe::Universe;
    for phase in 0..4 {
        let mut fixture = Fixture::debug(11);
        fixture.world.configure(&measured_config()).unwrap();
        match phase {
            0 => {
                fixture
                    .world
                    .animals
                    .get_mut(&id(1))
                    .unwrap()
                    .mind
                    .genome
                    .fail_mind = true
            }
            1 => fixture.action(MotorAction {
                thrust: f32::INFINITY,
                turn: 0.0,
            }),
            2 => fixture.world.mutation.fail = true,
            3 => fixture.world.mutation.reject_child = true,
            _ => unreachable!(),
        }
        let metrics = fixture.world.metrics.clone();
        let mut universe = Universe::from_world(fixture.world).unwrap();
        let original = encode(&universe);
        assert!(universe.advance().is_err());
        assert_eq!(encode(&universe), original);
        assert_eq!(universe.world().metrics, metrics);
    }
}

#[test]
fn recursive_births_create_complete_animals_without_running_new_minds() {
    let mut fixture = Fixture::debug(12);
    fixture.world.plants[0].position = Vec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 4.25;
    let report = fixture.tick();
    assert_eq!(
        report.events,
        [
            Event::Birth {
                parent: id(1),
                child: id(2)
            },
            Event::Birth {
                parent: id(2),
                child: id(3)
            },
            Event::Birth {
                parent: id(3),
                child: id(4)
            },
        ]
    );
    assert_eq!(report.decisions.len(), 1);
    assert_eq!(fixture.world.animals[&id(1)].mind.calls, 1);
    for child in [id(2), id(3), id(4)] {
        assert_eq!(fixture.world.animals[&child].mind.calls, 0);
    }
}

#[test]
fn offspring_inherit_a_mutated_genome_without_changing_the_parent() {
    let mut fixture = Fixture::debug(42);
    fixture.world.mutation.increment = 7;
    let allele = fixture.world.animals[&id(1)].mind.genome.allele;
    fixture.tick();
    assert_eq!(fixture.world.animals[&id(1)].mind.genome.allele, allele);
    assert_eq!(
        fixture.world.animals[&id(2)].mind.genome.allele,
        allele.wrapping_add(7)
    );
}

#[test]
fn failed_random_initialization_does_not_publish_consumed_randomness() {
    use crate::universe::Universe;

    let world: Ecosystem<FixedMind<true>, FixedGenome<true>, FixedMutation> =
        Ecosystem::empty(0, 1, 11, 17, None, FixedMutation::default());
    let mut universe = Universe::from_world(world).unwrap();
    let original = encode(&universe);
    assert!(
        universe
            .advance()
            .unwrap_err()
            .to_string()
            .contains("initialization failed")
    );
    assert_eq!(encode(&universe), original);
}

#[test]
fn immigrant_templates_bypass_random_initialization() {
    let genome = RnnGenome::default();
    let mut world: Ecosystem =
        Ecosystem::empty(0, 0, 42, 17, Some(genome.clone()), Mutation::default());
    let before = world.context.clone();
    world.admit(2).unwrap();
    assert!(
        world
            .animals
            .values()
            .all(|animal| animal.mind.brain.genome == genome)
    );
    assert_eq!(world.context, before);
}

#[test]
fn an_invalid_template_is_rejected_even_without_animals() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = f32::NAN;
    let world: Ecosystem = Ecosystem::empty(0, 0, 42, 17, Some(genome), Mutation::default());
    assert!(world.validate().is_err());
}

#[test]
fn discarding_a_tick_preserves_state_and_metrics() {
    let mut fixture = Fixture::debug(14);
    fixture.world.configure(&measured_config()).unwrap();
    let before = encode(&fixture.world);
    let metrics = fixture.world.metrics.clone();
    let (_, update) = fixture.world.compute_update(1).unwrap();
    drop(update);
    assert_eq!(encode(&fixture.world), before);
    assert_eq!(fixture.world.metrics, metrics);
}

#[test]
fn exhausted_allocators_cannot_publish_partial_ticks() {
    use crate::universe::Universe;

    for animals in [false, true] {
        let mut fixture = Fixture::debug(15);
        if animals {
            fixture.world.next_animal = u64::MAX;
        } else {
            fixture.world.next_plant = u64::MAX;
        }
        let mut universe = Universe::from_world(fixture.world).unwrap();
        let original = encode(&universe);
        assert!(universe.advance().is_err());
        assert_eq!(encode(&universe), original);
    }
}

fn measured_config<MC: Default>() -> Config<MC, (), FixedMutation> {
    let mut config = Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        metrics: MetricsConfig {
            enabled: true,
            window_ticks: 3,
        },
        ..Config::default()
    };
    config.animal.body.upkeep = UPKEEP;
    config.animal.body.energy_scale = 0.0;
    config
}

#[test]
fn template_validation_does_not_require_runtime_settings() {
    struct ConfiguredMind(RnnGenome);
    impl BrainStructure for ConfiguredMind {}
    impl From<RnnGenome> for ConfiguredMind {
        fn from(genome: RnnGenome) -> Self {
            Self(genome)
        }
    }
    impl AsRef<RnnGenome> for ConfiguredMind {
        fn as_ref(&self) -> &RnnGenome {
            &self.0
        }
    }
    impl Mind<GeneticContext> for ConfiguredMind {
        type Input = AnimalObservation;
        type Output = MotorAction;
        type Update = ();
        type Config = u8;
        fn configure(&mut self, config: &u8) -> Result<()> {
            ensure!(*config > 0, "default runtime settings are invalid");
            Ok(())
        }
        fn compute_update(
            &self,
            _: AnimalObservation,
            _: &mut GeneticContext,
        ) -> Result<(MotorAction, ())> {
            unreachable!("validation must not run inference")
        }
        fn apply_update(&mut self, _: ()) {}
    }
    let world = Ecosystem::<ConfiguredMind>::empty(
        0,
        0,
        42,
        17,
        Some(RnnGenome::default()),
        Mutation::default(),
    );
    assert!(world.mind_config.is_none());
    world.validate().unwrap();
}

#[test]
fn configured_immigration_is_identical_for_initial_and_later_arrivals() {
    let mut config: Config = Config {
        initial_animals: 1,
        minimum_population: 0,
        plants: 0,
        immigration: ImmigrationConfig {
            energy: 0.625,
            position: Placement::Fixed(Vec2::new(7.0, -9.0)),
        },
        ..Config::default()
    };
    config.animal.body.upkeep = 0.0075;
    config.animal.body.radius_scale = 4.0;
    config.genetics.initialization.hidden_units = 7;
    config.genetics.initialization.weight_min = -0.1;
    config.genetics.initialization.weight_max = 0.3;
    let initial = Ecosystem::new(config.clone(), 71).unwrap();
    config.initial_animals = 0;
    config.minimum_population = 1;
    let mut later = Ecosystem::new(config, 71).unwrap();
    let report = later.advance(1).unwrap();
    assert_eq!(report.events, [Event::Immigrant { id: id(1) }]);
    assert!(report.decisions.is_empty());
    let first = &initial.animals[&id(1)];
    let next = &later.animals[&id(1)];
    let mut expected_body = first.body.clone();
    expected_body.energy -= 0.0075 + 135.0e-6;
    assert_eq!(expected_body, next.body);
    assert_eq!(first.mind, next.mind);
    assert_eq!(next.mind.brain.memory.len(), 7);
    assert_eq!(initial.rng, later.rng);
    assert_eq!(initial.context, later.context);
}

#[test]
fn unequal_reproduction_conserves_energy() {
    let mut fixture = Fixture::debug(72);
    fixture.world.plants[0].position = Vec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 1.2;
    fixture.world.parameters.body.upkeep = 0.0;
    fixture.world.parameters.reproduction.child_energy_fraction = 0.25;
    fixture.tick();
    assert_eq!(fixture.world.animals.len(), 2);
    assert!((fixture.world.animals[&id(1)].body.energy - 0.9).abs() < f32::EPSILON);
    assert_eq!(fixture.world.animals[&id(2)].body.energy, 0.3);
}

#[test]
fn plant_growth_preserves_momentum_when_new_biomass_is_at_rest() {
    let mut config = Config {
        initial_animals: 0,
        minimum_population: 0,
        plants: 1,
        ..Config::default()
    };
    config.plant.initial_position = Placement::Fixed(Vec2::ZERO);
    config.plant.initial_energy = 0.25;
    config.plant.growth = 0.75;
    config.plant.linear_drag = 0.0;
    config.plant.angular_drag = 0.0;
    let mut world = Ecosystem::new(config, 72).unwrap();
    let plant = &mut world.plants[0];
    plant.velocity = Vec2::new(4.0, -2.0);
    plant.angular_velocity = 3.0;
    let mass_scale = world.parameters.physics.mass_per_energy;
    let momentum = plant.mass(mass_scale) * plant.velocity;
    let spin = plant.inertia(mass_scale) * plant.angular_velocity;
    world.advance(1).unwrap();
    let grown = &world.plants[0];
    assert_eq!(grown.energy, 1.0);
    assert!((grown.mass(mass_scale) * grown.velocity).distance(momentum) < 1e-12);
    assert!((grown.inertia(mass_scale) * grown.angular_velocity - spin).abs() < 1e-12);
    assert!(grown.position.length() > 0.0);
}

#[test]
fn replacement_growth_stops_at_the_configured_cap() {
    let mut config = measured_config::<()>();
    config.immigration.position = Placement::Fixed(Vec2::ZERO);
    config.plant.initial_position = Placement::Fixed(Vec2::new(10.0, 0.0));
    config.plant.replacement_position = Placement::Fixed(Vec2::new(100.0, 0.0));
    config.plant.initial_energy = 0.25;
    config.plant.replacement_energy = 0.125;
    config.plant.growth = 0.0625;
    config.plant.max_energy = 0.25;
    let mut world =
        Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(config, 72, None)
            .unwrap();
    world.advance(1).unwrap();
    assert_eq!(world.plants[0].position, Vec2::new(100.0, 0.0));
    assert_eq!(world.plants[0].energy, 0.1875);
    for tick in 2..=3 {
        world.advance(tick).unwrap();
        assert_eq!(world.plants[0].energy, 0.25);
    }
}

#[test]
fn sensing_applies_configured_scales_before_the_gradient_dead_zone() {
    let mut fixture = Fixture::debug(73);
    fixture.world.plants[0].position = Vec2::new(8.0, 0.0);
    fixture.world.parameters.sensing = SensingConfig {
        distance_scale: 0.125,
        signal_scale: 2.0,
        softening: 1.0,
        gradient_dead_zone: 0.3,
        ..SensingConfig::default()
    };
    let sensed = fixture.sense();
    assert_eq!(sensed.strength, 1.0);
    assert_eq!(sensed.gradient, Vec2::ZERO);
    fixture.world.parameters.sensing.signal_scale = 4.0;
    let amplified = fixture.sense();
    assert_eq!(amplified.strength, 2.0);
    assert_eq!(amplified.gradient, Vec2::X);
}

#[test]
fn sensing_scales_must_be_finite_and_positive() {
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let mut parameters = Parameters::default();
        parameters.sensing.distance_scale = invalid;
        assert!(parameters.validate().is_err());
        parameters.sensing = SensingConfig::default();
        parameters.sensing.signal_scale = invalid;
        assert!(parameters.validate().is_err());
        parameters.sensing = SensingConfig::default();
        parameters.sensing.velocity_scale = invalid;
        assert!(parameters.validate().is_err());
        parameters.sensing = SensingConfig::default();
        parameters.sensing.angular_velocity_scale = invalid;
        assert!(parameters.validate().is_err());
    }
}

#[test]
fn sensing_preserves_body_relative_signals_when_the_scene_rotates() {
    let mut fixture = Fixture::debug(73);
    fixture.world.parameters.sensing.velocity_scale = 0.25;
    fixture.world.parameters.sensing.angular_velocity_scale = 0.5;
    fixture.world.plants[0].position = Vec2::new(70.0, 35.0);
    fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(Vec2::new(-70.0, 35.0)),
                energy: 0.5,
            },
        )
        .unwrap();
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.velocity = Vec2::new(3.0, -4.0);
    body.angular_velocity = 1.5;
    let before = fixture.sense();
    assert_eq!(before.velocity, Vec2::new(0.75, -1.0));
    assert_eq!(before.angular_velocity, 0.75);

    fixture.world.plants[0].position = Vec2::new(-35.0, 70.0);
    fixture.world.animals.get_mut(&id(2)).unwrap().body.position = Vec2::new(-35.0, -70.0);
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.heading = std::f32::consts::FRAC_PI_2;
    body.velocity = Vec2::new(4.0, 3.0);
    let rotated = fixture.sense();
    assert!(rotated.gradient.distance(before.gradient) < 4.0 * f32::EPSILON);
    assert!(before.animal_strength > 0.0);
    assert!(rotated.animal_gradient.distance(before.animal_gradient) < 4.0 * f32::EPSILON);
    assert_eq!(rotated.animal_strength, before.animal_strength);
    assert!(
        rotated.velocity.distance(before.velocity) < 4.0 * f32::EPSILON * before.velocity.length()
    );
    assert_eq!(rotated.strength, before.strength);
    assert_eq!(rotated.angular_velocity, before.angular_velocity);
}

#[test]
fn physics_substeps_do_not_repeat_thinking_upkeep_or_plant_growth() {
    for substeps in [1, 8, 32] {
        let mut fixture = Fixture::debug(74);
        fixture.world.parameters.physics.substeps = substeps;
        fixture.world.plants[0].position = Vec2::new(100.0, 0.0);
        fixture.world.plants[0].energy = 0.25;
        fixture.action(MotorAction {
            thrust: 0.5,
            turn: 0.25,
        });
        let report = fixture.tick();
        assert!(report.events.is_empty());
        assert_eq!(report.decisions.len(), 1);
        assert_eq!(fixture.world.animals[&id(1)].mind.calls, 1);
        assert_eq!(fixture.world.animals[&id(1)].body.energy, 0.975 - UPKEEP);
        assert_eq!(fixture.world.plants[0].energy, 0.2525);
    }
}

#[test]
fn motor_cost_is_paid_at_a_wall_before_reproduction() {
    let mut fixture = Fixture::debug(75);
    fixture.world.plants[0].position = Vec2::ZERO;
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.energy = 1.001;
    body.position.x = HALF_SIZE.x - body.radius();
    fixture.world.parameters.body.upkeep = 0.0;
    fixture.world.parameters.body.max_acceleration = 10.0;
    fixture.world.parameters.body.energy_scale = 0.001;
    fixture.world.parameters.body.force_loss = 1.0;
    fixture.action(MotorAction {
        thrust: 1.0,
        turn: 0.0,
    });
    let report = fixture.tick();
    let decision = &report.decisions[0];
    assert!(decision.movement_energy > 0.001);
    assert!(
        report.events.is_empty(),
        "motor energy must be paid before splitting"
    );
    assert_eq!(
        fixture.world.animals[&id(1)].body.energy,
        1.001 - decision.movement_energy
    );
}

#[test]
fn coasting_does_not_spend_motor_energy() {
    let mut fixture = Fixture::debug(76);
    fixture.world.plants[0].position = Vec2::new(100.0, 100.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.velocity = Vec2::new(5.0, 0.0);
    fixture.world.parameters.body.upkeep = 0.0;
    fixture.world.parameters.body.energy_scale = 0.001;
    let report = fixture.tick();
    assert!(report.decisions[0].distance > 0.0);
    assert_eq!(report.decisions[0].movement_energy, 0.0);
    assert_eq!(fixture.world.animals[&id(1)].body.energy, 0.975);
}

#[test]
fn work_conversion_scales_before_dividing_by_tiny_efficiency() {
    for energy_scale in [0.0, 1e-40] {
        let mut fixture = Fixture::debug(76);
        fixture.world.plants[0].position = Vec2::new(100.0, 100.0);
        fixture.world.parameters.body.energy_scale = energy_scale;
        fixture.world.parameters.body.motor_efficiency = 1e-38;
        if energy_scale == 0.0 {
            fixture.world.parameters.body.force_loss = f32::MAX;
        }
        fixture.action(MotorAction {
            thrust: 1.0,
            turn: 0.0,
        });
        fixture.world.validate().unwrap();
        let report = fixture.tick();
        let cost = report.decisions[0].movement_energy;
        assert!(cost.is_finite());
        assert_eq!(cost == 0.0, energy_scale == 0.0);
    }
}

#[test]
fn splitting_preserves_both_bodies_motion_without_an_extra_energy_fee() {
    let mut fixture = Fixture::debug(77);
    fixture.world.plants[0].position = Vec2::new(100.0, 100.0);
    fixture.world.parameters.body.upkeep = 0.0;
    fixture.world.parameters.body.energy_scale = 1.0;
    fixture.world.parameters.body.linear_drag = 0.0;
    fixture.world.parameters.body.angular_drag = 0.0;
    fixture.world.parameters.physics.time_step = 0.5;
    fixture.world.parameters.physics.substeps = 1;
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.energy = 1.2;
    body.heading = 0.3;
    body.velocity = Vec2::new(3.0, -2.0);
    body.angular_velocity = 0.4;
    let report = fixture.tick();
    assert_eq!(
        report.events,
        [Event::Birth {
            parent: id(1),
            child: id(2)
        }]
    );
    assert_eq!(report.decisions[0].movement_energy, 0.0);
    let first = &fixture.world.animals[&id(1)].body;
    let second = &fixture.world.animals[&id(2)].body;
    assert!(first.position.distance(second.position) > 0.0);
    assert!(
        ((first.position + second.position) * 0.5).distance(report.decisions[0].position_after)
            < 2.0 * f32::EPSILON * (first.position.length() + second.position.length())
    );
    for animal in fixture.world.animals.values() {
        assert_eq!(animal.body.heading, report.decisions[0].heading_after);
        assert_eq!(animal.body.velocity, Vec2::new(3.0, -2.0));
        assert_eq!(animal.body.angular_velocity, 0.4);
        assert_eq!(animal.body.energy, 0.6);
    }
}

#[test]
fn invalid_physics_and_conflicting_restore_parameters_are_rejected() {
    use crate::universe::Universe;
    let config = Config {
        initial_animals: 0,
        plants: 0,
        minimum_population: 0,
        ..Config::default()
    };
    let invalid_changes: &[fn(&mut Config)] = &[
        |c| c.world.width = 0.0,
        |c| c.world.height = f32::INFINITY,
        |c| c.immigration.energy = -0.0025,
        |c| c.immigration.position = Placement::Fixed(Vec2::splat(10000.0)),
        |c| c.animal.reproduction.threshold = 0.0,
        |c| c.animal.reproduction.child_energy_fraction = 1.0,
        |c| c.animal.reproduction.child_energy_fraction = 0.0,
        |c| c.animal.body.upkeep = -0.0025,
        |c| c.physics.mass_per_energy = 0.0,
        |c| c.physics.mass_per_energy = f32::NAN,
        |c| c.physics.contact_frequency = 0.0,
        |c| c.physics.contact_damping = -1.0,
        |c| c.physics.max_placement_attempts = 0,
        |c| c.animal.body.max_acceleration = f32::INFINITY,
        |c| c.animal.body.max_angular_acceleration = -1.0,
        |c| c.animal.body.linear_drag = -1.0,
        |c| c.animal.body.angular_drag = f32::INFINITY,
        |c| c.physics.time_step = 0.0,
        |c| c.physics.substeps = 0,
        |c| c.animal.body.energy_scale = -0.1,
        |c| c.animal.body.energy_scale = f32::MAX,
        |c| c.animal.body.motor_efficiency = 0.0,
        |c| c.animal.body.motor_efficiency = 1.01,
        |c| c.animal.body.force_loss = -1.0,
        |c| c.animal.body.torque_loss = f32::NAN,
        |c| c.plant.max_energy = 0.0,
        |c| c.plant.initial_energy = 1.0025,
        |c| c.plant.radius_scale = 0.0,
        |c| c.plant.linear_drag = -1.0,
        |c| c.plant.angular_drag = f32::NAN,
        |c| c.animal.body.radius_scale = 0.0,
        |c| c.animal.brain_weight_upkeep = -0.1,
        |c| c.sensing.softening = 0.0,
        |c| c.sensing.animal_signal_scale = -0.1,
        |c| c.sensing.animal_signal_scale = f32::NAN,
        |c| c.sensing.animal_signal_scale = f32::INFINITY,
        |c| c.genetics.initialization.hidden_units = 0,
        |c| c.genetics.mutation.log_sigma_std = f32::NAN,
    ];
    for change in invalid_changes {
        let mut invalid = config.clone();
        change(&mut invalid);
        assert!(Ecosystem::new(invalid, 74).is_err());
    }
    let universe = Universe::new(config.clone(), 74).unwrap();
    let mut changed = config.clone();
    changed.animal.body.energy_scale = 0.1;
    assert!(universe.restore(changed).is_err());
    let mut world = Ecosystem::new(config, 74).unwrap();
    world.parameters.world.height = 0.0;
    assert!(world.validate().is_err());
}

#[test]
fn metrics_are_pure_and_do_not_change_reports_or_checkpoint_bytes() {
    use crate::metrics::Metrics;
    let mut config: Config = crate::test_support::ecosystem("world/debug");
    config.metrics.enabled = false;
    config.animal.body.energy_scale = 1e-5;
    config.plant.replenishment = Some(ReplenishmentConfig::default());
    let mut measured_config = config.clone();
    measured_config.metrics = MetricsConfig {
        enabled: true,
        window_ticks: 7,
    };
    let mut plain = Ecosystem::new(config, 42).unwrap();
    let mut measured = Ecosystem::new(measured_config, 42).unwrap();
    for tick in 1..=120 {
        assert_eq!(
            plain.advance(tick).unwrap(),
            measured.advance(tick).unwrap()
        );
        assert_eq!(encode(&plain), encode(&measured), "tick {tick}");
        let summary = measured.metrics();
        assert_eq!(measured.metrics(), summary);
        assert_eq!(summary.population, Some(measured.animals.len() as u64));
        assert_eq!(plain.metrics(), Summary::default());
    }
}

#[test]
fn rates_account_for_births_deaths_and_empty_exposure() {
    use crate::metrics::Metrics;
    let mut fixture = Fixture::debug(43);
    fixture.world.configure(&measured_config()).unwrap();
    let initial = fixture.world.metrics();
    assert_eq!(initial.population, Some(1));
    assert!(initial.food_per_tick.is_none());
    fixture.world.advance(1).unwrap();
    let first = fixture.world.metrics();
    assert_eq!(first.births_per_tick, Some(1.0));
    assert_eq!(first.food_per_tick, Some(f64::from(0.1_f32)));
    assert_eq!(first.window.unwrap().active_animal_ticks, 1);
    fixture.world.advance(2).unwrap();
    let second = fixture.world.metrics();
    assert_eq!(second.births_per_tick, Some(0.5));
    assert_eq!(second.window.unwrap().active_animal_ticks, 3);
    fixture.world.advance(3).unwrap();
    assert!(fixture.world.metrics_window().unwrap().complete);
    fixture.world.advance(4).unwrap();
    assert_eq!(fixture.world.metrics_window().unwrap().start_tick, 3);
    assert_eq!(fixture.world.metrics().births_per_tick, Some(0.0));

    let mut fixture = Fixture::debug(46);
    fixture.world.plants[0].position.x = 100.0;
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 0.0025;
    fixture.world.configure(&measured_config()).unwrap();
    fixture.world.advance(1).unwrap();
    fixture.world.advance(2).unwrap();
    let summary = fixture.world.metrics();
    assert_eq!(summary.population, Some(0));
    assert_eq!(summary.mean_animal_energy, None);
    assert_eq!(summary.deaths_per_tick, Some(0.5));
    assert_eq!(summary.window.unwrap().active_animal_ticks, 1);
    fixture.world.advance(3).unwrap();
    fixture.world.advance(4).unwrap();
    assert_eq!(fixture.world.metrics().distance_per_animal_tick, None);
    assert_eq!(fixture.world.metrics().food_per_tick, Some(0.0));
}

#[test]
fn complete_and_partial_windows_preserve_rate_units() {
    use crate::metrics::Metrics;
    for window_ticks in [1, 3, 10] {
        let mut fixture = Fixture::debug(43);
        let mut config = measured_config();
        config.metrics.window_ticks = window_ticks;
        config.animal.body.energy_scale = 1e-6;
        fixture.world.parameters.body.energy_scale = config.animal.body.energy_scale;
        fixture.world.configure(&config).unwrap();
        fixture.action(MotorAction {
            thrust: 1.0,
            turn: 0.0,
        });
        let (mut births, mut distance, mut reported_distance) = (0.0, 0.0, 0.0);
        let (mut movement_energy, mut reported_energy) = (0.0, 0.0);
        for tick in 1..=3 {
            let report = fixture.tick();
            reported_distance += report
                .decisions
                .iter()
                .map(|decision| f64::from(decision.distance))
                .sum::<f64>();
            reported_energy += report
                .decisions
                .iter()
                .map(|decision| f64::from(decision.movement_energy))
                .sum::<f64>();
            let summary = fixture.world.metrics();
            let window = summary.window.unwrap();
            if window.complete || tick == 3 {
                births += summary.births_per_tick.unwrap() * window.samples as f64;
                distance +=
                    summary.distance_per_animal_tick.unwrap() * window.active_animal_ticks as f64;
                movement_energy +=
                    summary.movement_energy_per_tick.unwrap() * window.samples as f64;
            }
        }
        assert_eq!(births, 1.0);
        assert!(reported_distance > 0.0);
        assert!((distance - reported_distance).abs() < 1e-12);
        assert!(reported_energy > 0.0);
        assert!((movement_energy - reported_energy).abs() < 1e-12);
    }
}

#[test]
fn resuming_starts_a_fresh_metrics_window() {
    use crate::universe::Universe;
    let config: Config = crate::test_support::ecosystem("world/debug");
    let mut original = Universe::new(config.clone(), 44).unwrap();
    original.advance().unwrap();
    let decoded: Universe = ciborium::from_reader(encode(&original).as_slice()).unwrap();
    let mut resumed = decoded.restore(config).unwrap();
    assert!(resumed.universe().read().world().metrics_window().is_none());
    resumed.tick().unwrap();
    let window = resumed.universe().read().world().metrics_window().unwrap();
    assert_eq!(
        (window.start_tick, window.end_tick, window.samples),
        (1, 2, 1)
    );
}

#[test]
fn brain_upkeep_charges_parent_and_resized_child_once_at_their_own_sizes() {
    let mut config: Config = Config {
        initial_animals: 1,
        minimum_population: 0,
        plants: 0,
        ..Default::default()
    };
    config.animal.body.upkeep = 0.0;
    config.animal.body.energy_scale = 0.0;
    config.animal.brain_weight_upkeep = 0.0001;
    config.metrics.window_ticks = 1;
    config.immigration.energy = 1.2;
    config.genetics.mutation.size_probability = 1.0;
    let genome = RnnGenome {
        hidden_units: 2,
        weights: vec![0.0; 30],
        ..Default::default()
    };
    let mut world: Ecosystem = Ecosystem::with_genetics(config, 93, Some(genome)).unwrap();
    let report = world.advance(1).unwrap();
    assert_eq!(world.animals.len(), 2);
    assert_eq!(
        crate::metrics::Metrics::metrics(&world).brain_upkeep_energy_per_tick,
        Some(report.brain_upkeep_energy)
    );
    let parent = &world.animals[&id(1)];
    let child = &world.animals[&id(2)];
    let child_weights = match child.mind.brain.genome.hidden_units {
        1 => 15.0,
        3 => 47.0,
        _ => panic!("size must change by one"),
    };
    assert!((parent.body.energy - (0.6 - 30.0 * 0.0001)).abs() < f32::EPSILON);
    assert!((child.body.energy - (0.6 - child_weights * 0.0001)).abs() < f32::EPSILON);
    assert!((report.brain_upkeep_energy - (30.0 + child_weights as f64) * 0.0001).abs() < 1e-9);
}

#[test]
fn hidden_size_metrics_summarize_the_current_population_and_omit_empty_means() {
    use crate::metrics::Metrics;
    let mut world = Ecosystem::new(
        Config {
            initial_animals: 0,
            minimum_population: 0,
            plants: 0,
            ..Default::default()
        },
        94,
    )
    .unwrap();
    let empty = world.metrics();
    assert_eq!(
        (
            empty.min_hidden_units,
            empty.mean_hidden_units,
            empty.max_hidden_units
        ),
        (None, None, None)
    );
    for (serial, hidden_units, weights) in [(1, 2, 30), (2, 4, 66)] {
        world.animals.insert(
            id(serial),
            Animal {
                body: Body::new(Vec2::ZERO, 0.5),
                mind: AnimalMind::new(RnnGenome {
                    hidden_units,
                    weights: vec![0.0; weights],
                    ..Default::default()
                })
                .unwrap(),
            },
        );
    }
    let summary = world.metrics();
    assert_eq!(
        (
            summary.min_hidden_units,
            summary.mean_hidden_units,
            summary.max_hidden_units
        ),
        (Some(2), Some(3.0), Some(4))
    );
    let mut fixture = Fixture::debug(94);
    fixture.world.metrics = metrics::Cache::new(&measured_config::<()>());
    assert_eq!(fixture.world.metrics().mean_hidden_units, None);
}

#[test]
fn animal_field_excludes_self_and_dead_bodies_and_weights_other_energy() {
    let mut fixture = Fixture::debug(95);
    let alone = fixture.sense();
    assert_eq!(alone.animal_strength, 0.0);
    assert_eq!(alone.animal_gradient, Vec2::ZERO);
    fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(Vec2::new(70.0, 0.0)),
                energy: 0.5,
            },
        )
        .unwrap();
    let other = fixture.sense();
    assert!((other.animal_strength - 0.025 / 0.11).abs() < f32::EPSILON);
    assert_eq!(other.animal_gradient, Vec2::X);
    assert_eq!(
        (other.strength, other.gradient),
        (alone.strength, alone.gradient)
    );
    fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(Vec2::new(-70.0, 0.0)),
                energy: 0.0,
            },
        )
        .unwrap();
    for energy in [0.0, -0.5] {
        fixture.world.animals.get_mut(&id(3)).unwrap().body.energy = energy;
        assert_eq!(fixture.sense(), other);
    }
    fixture.world.parameters.sensing.animal_signal_scale = 0.0;
    assert_eq!(fixture.sense(), alone);
}

#[test]
fn turning_in_place_rotates_both_gradient_axes_but_not_strengths() {
    let mut fixture = Fixture::debug(96);
    fixture.world.plants[0].position = Vec2::new(0.0, 70.0);
    fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(Vec2::new(70.0, 0.0)),
                energy: 0.5,
            },
        )
        .unwrap();
    let before = fixture.sense();
    assert_eq!(before.gradient, Vec2::Y);
    assert_eq!(before.animal_gradient, Vec2::X);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.heading = std::f32::consts::FRAC_PI_2;
    let turned = fixture.sense();
    assert!(turned.gradient.distance(Vec2::X) < f32::EPSILON);
    assert!(turned.animal_gradient.distance(-Vec2::Y) < f32::EPSILON);
    assert_eq!(turned.strength, before.strength);
    assert_eq!(turned.animal_strength, before.animal_strength);
}
