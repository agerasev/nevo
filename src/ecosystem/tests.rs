use super::*;
use crate::genome::{Genome, Mutate, RandomInit};

// Reference defaults in normalized energy units; spatial units are unchanged.
const HALF_SIZE: DVec2 = DVec2::new(630.0, 350.0);
const START_ENERGY: f64 = 0.25;
const RIPE_ENERGY: f64 = 1.0;
const UPKEEP: f64 = 0.005;

#[derive(Clone, Copy)]
struct Admission {
    position: Option<DVec2>,
    energy: f64,
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
    M: Mind<GeneticContext, Input = FoodObservation, Output = MoveAction> + AsRef<G> + TryFrom<G>,
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
        result
    }
    fn seed_plants_at(&mut self, position: Option<DVec2>) -> Result<()> {
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
    action: MoveAction,
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
    configurations: usize,
}

impl<const FAIL_INIT: bool> AsRef<FixedGenome<FAIL_INIT>> for FixedMind<FAIL_INIT> {
    fn as_ref(&self) -> &FixedGenome<FAIL_INIT> {
        &self.genome
    }
}

impl<const FAIL_INIT: bool> TryFrom<FixedGenome<FAIL_INIT>> for FixedMind<FAIL_INIT> {
    type Error = anyhow::Error;

    fn try_from(genome: FixedGenome<FAIL_INIT>) -> Result<Self> {
        ensure!(!genome.reject_mind, "mind construction failed");
        Ok(Self {
            genome,
            calls: 0,
            configurations: 0,
        })
    }
}

impl<const FAIL_INIT: bool> Mind<GeneticContext> for FixedMind<FAIL_INIT> {
    type Input = FoodObservation;
    type Output = MoveAction;
    type Update = usize;
    type Config = ();

    fn configure(&mut self, _: &()) -> Result<()> {
        self.configurations += 1;
        Ok(())
    }

    fn compute_update(
        &self,
        _: FoodObservation,
        cx: &mut GeneticContext,
    ) -> Result<(MoveAction, usize)> {
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
        world
            .admit_with(
                1,
                Admission {
                    position: Some(DVec2::ZERO),
                    energy: 0.975,
                },
            )
            .unwrap();
        world.seed_plants_at(Some(DVec2::ZERO)).unwrap();
        Self { world, number: 0 }
    }

    fn tick(&mut self) -> TickReport {
        self.number += 1;
        self.world.advance(self.number).unwrap()
    }

    fn action(&mut self, action: MoveAction) {
        for animal in self.world.animals.values_mut() {
            animal.mind.genome.action = action;
        }
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
    fixture.world.animals.get_mut(&first).unwrap().body.energy = START_ENERGY;
    let second = fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(DVec2::ZERO),
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
    fixture.world.plants[0].position = DVec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 0.0025;

    let starving = fixture.tick();
    assert!(starving.events.is_empty());
    assert_eq!(starving.decisions[0].energy, 0.0025);
    assert_eq!(fixture.world.animals[&id(1)].body.energy, -0.0025);
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
fn field_uses_maturity_distance_and_the_world_axes() {
    let mut fixture = Fixture::debug(3);
    fixture.world.plants[0].position = DVec2::new(70.0, 0.0);
    fixture
        .world
        .add_plant(DVec2::new(-70.0, 0.0), RIPE_ENERGY)
        .unwrap();
    let balanced = fixture.world.sense(DVec2::ZERO);
    assert_eq!(balanced.gradient, DVec2::ZERO);
    assert!((balanced.strength - 1.0 / 0.11).abs() < 1e-12);
    fixture.world.plants[1].energy *= 0.5;
    let unbalanced = fixture.world.sense(DVec2::ZERO);
    assert!((unbalanced.gradient - DVec2::X).length() < 1e-12);
    assert!((unbalanced.strength - 0.75 / 0.11).abs() < 1e-12);
}

#[test]
fn body_stops_at_the_edge_and_cannot_eat_across_it() {
    let mut fixture = Fixture::debug(4);
    fixture.action(MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    });
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.position.x = HALF_SIZE.x - 5.0;
    body.energy = START_ENERGY;
    fixture.world.plants[0].position.x = -HALF_SIZE.x;
    let tick = fixture.tick();
    assert!(tick.events.is_empty());
    let body = &fixture.world.animals[&id(1)].body;
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - 5.0, 0.0));
    assert!(body.position.x + body.radius() <= HALF_SIZE.x);
}

#[test]
fn parent_senses_ripe_food_and_newborn_waits_to_process_it() {
    use crate::mind::rnn::RnnGenome;

    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    let mut world: Ecosystem =
        Ecosystem::with_genetics(crate::test_support::ecosystem("debug"), 7, Some(genome)).unwrap();
    let before = world.clone();
    let sensed = world.sense(world.animals[&id(1)].body.position);
    assert_eq!(world, before, "sensing must not change the world");
    assert_eq!(sensed.strength, 100.0);

    let mut expected_genome = world.animals[&id(1)].mind.brain.genome.clone();
    let mut expected_context = world.context.clone();
    expected_genome
        .mutate(&WeightMutation::default(), &mut expected_context)
        .unwrap();
    let first = world.advance(1).unwrap();
    assert_eq!(
        world.animals[&id(1)].mind.brain.genome,
        before.animals[&id(1)].mind.brain.genome
    );
    assert_eq!(world.animals[&id(2)].mind.brain.genome, expected_genome);
    assert_eq!(world.context, expected_context);
    assert_eq!(first.decisions.len(), 1);
    assert_eq!(first.decisions[0].id, id(1));
    assert_eq!(first.decisions[0].energy, 0.975);
    assert_eq!(first.decisions[0].observation, sensed);
    assert_eq!(world.animals[&id(1)].body.energy, 0.5325);
    assert_eq!(world.animals[&id(2)].body.energy, 0.5325);
    assert_ne!(world.animals[&id(1)].mind.brain.memory, [0.0; 4]);
    assert_eq!(world.animals[&id(2)].mind.brain.memory, [0.0; 4]);
    let second = world.advance(2).unwrap();
    assert_eq!(
        second
            .decisions
            .iter()
            .map(|decision| decision.id)
            .collect::<Vec<_>>(),
        [id(1), id(2)]
    );
    assert_ne!(world.animals[&id(2)].mind.brain.memory, [0.0; 4]);
}

#[test]
fn movement_enters_contact_before_feeding_and_birth() {
    let mut fixture = Fixture::debug(8);
    fixture.action(MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    });
    fixture.world.plants[0].position = DVec2::new(20.0, 0.0);
    let body = &fixture.world.animals[&id(1)].body;
    assert!(
        body.position.distance(fixture.world.plants[0].position)
            >= 0.8 * (body.radius() + fixture.world.plants[0].radius())
    );
    let report = fixture.tick();
    assert_eq!(report.decisions.len(), 1);
    assert_eq!(report.decisions[0].position_before, DVec2::ZERO);
    assert_eq!(report.decisions[0].position_after, DVec2::new(10.0, 0.0));
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
    assert_eq!(
        (fixture.world.animals[&id(1)].body.position + fixture.world.animals[&id(2)].body.position)
            * 0.5,
        report.decisions[0].position_after
    );
}

#[test]
fn feeding_growth_cannot_leave_a_body_outside_the_arena() {
    let mut fixture = Fixture::debug(9);
    let body = &mut fixture.world.animals.get_mut(&id(1)).unwrap().body;
    body.energy = START_ENERGY;
    body.position = DVec2::new(HALF_SIZE.x - body.radius(), 0.0);
    fixture.world.plants[0].position = body.position;
    let report = fixture.tick();
    let body = &fixture.world.animals[&id(1)].body;
    assert_eq!(body.energy, 0.345);
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - body.radius(), 0.0));
    assert!(body.position.x < report.decisions[0].position_after.x);
}

#[test]
fn replacements_follow_culling_and_precede_births() {
    let mut fixture = Fixture::debug(10);
    fixture
        .world
        .admit_with(
            1,
            Admission {
                position: Some(DVec2::ZERO),
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
            1 => fixture.action(MoveAction {
                direction: DVec2::new(f64::INFINITY, 0.0),
                speed_fraction: 0.0,
            }),
            2 => fixture.world.mutation.fail = true,
            3 => fixture.world.mutation.reject_child = true,
            _ => unreachable!(),
        }
        let mut universe = Universe::from_world(fixture.world).unwrap();
        let original = encode(&universe);
        assert!(universe.advance().is_err());
        assert_eq!(encode(&universe), original);
    }
}

#[test]
fn initial_and_later_arrivals_use_the_same_admission_and_rng_sequence() {
    let mut initial = Ecosystem::<AnimalMind>::empty(0, 0, 42, 17, None, WeightMutation::default());
    let arrivals = initial.admit_with(1, Admission::default()).unwrap();
    let mut later = Ecosystem::<AnimalMind>::empty(0, 1, 42, 17, None, WeightMutation::default());
    let report = later.advance(1).unwrap();
    assert_eq!(arrivals, vec![id(1)]);
    assert_eq!(
        initial.animals[&id(1)].body.position,
        later.animals[&id(1)].body.position
    );
    assert_eq!(initial.animals[&id(1)].mind, later.animals[&id(1)].mind);
    assert_eq!(initial.rng, later.rng);
    assert_eq!(initial.context, later.context);
    assert_eq!(initial.next_animal, later.next_animal);
    assert_eq!(
        initial.animals[&id(1)].body.energy - UPKEEP,
        later.animals[&id(1)].body.energy
    );
    assert_eq!(report.events, [Event::Immigrant { id: id(1) }]);
    assert!(report.decisions.is_empty());
}

#[test]
fn recursive_births_create_complete_animals_without_running_new_minds() {
    let mut fixture = Fixture::debug(12);
    fixture.world.plants[0].position = DVec2::new(100.0, 0.0);
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
fn identities_serialize_as_integers_and_never_wrap() {
    let mut fixture = Fixture::debug(13);
    let mut bytes = Vec::new();
    ciborium::into_writer(&id(1), &mut bytes).unwrap();
    assert_eq!(
        ciborium::from_reader::<u64, _>(bytes.as_slice()).unwrap(),
        1
    );
    fixture.world.next_animal = u64::MAX;
    assert!(fixture.world.admit_with(1, Admission::default()).is_err());
    assert_eq!(fixture.world.next_animal, u64::MAX);
    assert_eq!(fixture.world.animals.len(), 1);
}

#[test]
fn a_different_genome_and_mutation_type_drive_generic_reproduction() {
    let mut world: Ecosystem<FixedMind, FixedGenome, FixedMutation> = Ecosystem::empty(
        0,
        0,
        42,
        17,
        None,
        FixedMutation {
            increment: 7,
            ..FixedMutation::default()
        },
    );
    let mut expected_rng = world.context.rng.clone();
    let parent_allele = expected_rng.next_u64();
    let parent = world
        .admit_with(
            1,
            Admission {
                position: Some(DVec2::ZERO),
                energy: 1.25,
            },
        )
        .unwrap()[0];
    assert_eq!(world.animals[&parent].mind.genome.allele, parent_allele);
    assert_eq!(world.context.rng, expected_rng);

    let report = world.advance(1).unwrap();
    let child = match report.events.as_slice() {
        [
            Event::Birth {
                parent: actual,
                child,
            },
        ] => {
            assert_eq!(*actual, parent);
            *child
        }
        events => panic!("unexpected events: {events:?}"),
    };
    assert_eq!(world.animals[&parent].mind.genome.allele, parent_allele);
    assert_eq!(
        world.animals[&child].mind.genome.allele,
        parent_allele.wrapping_add(7)
    );
    assert_eq!(world.animals[&parent].mind.calls, 1);
    assert_eq!(world.animals[&child].mind.calls, 0);
    assert_eq!(
        world.context.rng, expected_rng,
        "this mutation is deterministic"
    );
    assert_eq!(world.animals[&parent].body.energy, 0.62);
    assert_eq!(world.animals[&child].body.energy, 0.62);
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
fn immigrant_template_applies_to_every_arrival_without_genetic_draws() {
    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    let mut world: Ecosystem = Ecosystem::empty(
        0,
        0,
        42,
        17,
        Some(genome.clone()),
        WeightMutation::default(),
    );
    let original_context = world.context.clone();
    let admission = Admission {
        position: Some(DVec2::ZERO),
        energy: 0.25,
    };
    let first = world.admit_with(1, admission).unwrap()[0];
    world.advance(1).unwrap();
    assert_ne!(world.animals[&first].mind.brain.memory, [0.0; 4]);
    let later = world.admit_with(1, admission).unwrap()[0];
    for animal in world.animals.values() {
        assert_eq!(animal.mind.brain.genome, genome);
    }
    assert_eq!(world.animals[&later].mind.brain.memory, [0.0; 4]);
    assert_eq!(world.context, original_context);
}

#[test]
fn an_invalid_template_is_rejected_even_without_animals() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = f32::NAN;
    let world: Ecosystem = Ecosystem::empty(0, 0, 42, 17, Some(genome), WeightMutation::default());
    assert!(world.validate().is_err());
}

#[test]
fn preparing_and_discarding_a_tick_leaves_every_component_unchanged() {
    let fixture = Fixture::debug(14);
    let before = encode(&fixture.world);
    let (first, update) = fixture.world.compute_update(1).unwrap();
    assert_eq!(encode(&fixture.world), before);
    assert!(
        first
            .events
            .iter()
            .any(|event| matches!(event, Event::Birth { .. }))
    );
    drop(update);
    let (repeated, update) = fixture.world.compute_update(1).unwrap();
    assert_eq!(repeated, first);
    let mut world = fixture.world;
    world.apply_update(update);
    assert_eq!(world.animals[&id(1)].mind.calls, 1);
    assert_eq!(world.animals[&id(2)].mind.calls, 0);
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
    Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        metrics: MetricsConfig {
            enabled: true,
            window_ticks: 3,
        },
        ..Config::default()
    }
}

#[test]
fn template_validation_does_not_require_deserialized_runtime_settings() {
    use crate::universe::Universe;

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct ConfiguredMind {
        genome: RnnGenome,
        #[serde(skip)]
        configured: bool,
    }
    impl From<RnnGenome> for ConfiguredMind {
        fn from(genome: RnnGenome) -> Self {
            Self {
                genome,
                configured: false,
            }
        }
    }
    impl AsRef<RnnGenome> for ConfiguredMind {
        fn as_ref(&self) -> &RnnGenome {
            &self.genome
        }
    }
    impl Mind<GeneticContext> for ConfiguredMind {
        type Input = FoodObservation;
        type Output = MoveAction;
        type Update = ();
        type Config = u8;
        fn configure(&mut self, config: &u8) -> Result<()> {
            ensure!(*config > 0, "the default runtime configuration is invalid");
            self.configured = true;
            Ok(())
        }
        fn compute_update(
            &self,
            _: FoodObservation,
            _: &mut GeneticContext,
        ) -> Result<(MoveAction, ())> {
            ensure!(self.configured, "mind was not configured");
            Ok((MoveAction::default(), ()))
        }
        fn apply_update(&mut self, _: ()) {}
    }
    let mut config = Config::<u8> {
        initial_animals: 0,
        plants: 0,
        minimum_population: 1,
        ..Config::default()
    };
    config.animal.mind = 7;
    let world =
        Ecosystem::<ConfiguredMind>::with_genetics(config, 52, Some(RnnGenome::default())).unwrap();
    world.validate().unwrap();
    let original = Universe::from_world(world).unwrap();
    let mut bytes = Vec::new();
    ciborium::into_writer(&original, &mut bytes).unwrap();
    let decoded: Universe<Ecosystem<ConfiguredMind>> =
        ciborium::from_reader(bytes.as_slice()).unwrap();
    assert!(decoded.world().mind_config.is_none());
    decoded.validate().unwrap();
    let mut resumed = decoded.restore(config).unwrap();
    resumed.tick().unwrap(); // The immigrant receives the supplied valid settings.
    resumed.tick().unwrap(); // Its first computation requires those settings.
    assert!(
        resumed.universe().read().world().animals[&id(1)]
            .mind
            .configured
    );
}

#[test]
fn configured_immigration_is_identical_for_initial_and_later_arrivals() {
    let mut config: Config = Config {
        initial_animals: 1,
        minimum_population: 0,
        plants: 0,
        immigration: ImmigrationConfig {
            energy: 0.625,
            position: Placement::Fixed(DVec2::new(7.0, -9.0)),
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
    assert_eq!(first.body.position, next.body.position);
    assert_eq!(first.body.energy - 0.0075, next.body.energy);
    assert_eq!(first.body.radius_scale, next.body.radius_scale);
    assert_eq!(first.mind, next.mind);
    assert_eq!(next.mind.brain.memory.len(), 7);
    assert_eq!(initial.rng, later.rng);
    assert_eq!(initial.context, later.context);
}

#[test]
fn unequal_reproduction_conserves_energy() {
    let mut fixture = Fixture::debug(72);
    fixture.world.plants[0].position = DVec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 1.2;
    fixture.world.parameters.body.upkeep = 0.0;
    fixture.world.parameters.reproduction.child_energy_fraction = 0.25;
    fixture.tick();
    assert_eq!(fixture.world.animals.len(), 2);
    assert!((fixture.world.animals[&id(1)].body.energy - 0.9).abs() < 1e-12);
    assert_eq!(fixture.world.animals[&id(2)].body.energy, 0.3);
}

#[test]
fn replacement_growth_stops_at_the_configured_cap() {
    let mut config = measured_config::<()>();
    config.immigration.position = Placement::Fixed(DVec2::ZERO);
    config.plant.initial_position = Placement::Fixed(DVec2::ZERO);
    config.plant.replacement_position = Placement::Fixed(DVec2::new(100.0, 0.0));
    config.plant.initial_energy = 0.25;
    config.plant.replacement_energy = 0.125;
    config.plant.growth = 0.0625;
    config.plant.max_energy = 0.25;
    let mut world =
        Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(config, 72, None)
            .unwrap();
    world.advance(1).unwrap();
    assert_eq!(world.plants[0].position, DVec2::new(100.0, 0.0));
    assert_eq!(world.plants[0].energy, 0.1875);
    for tick in 2..=3 {
        world.advance(tick).unwrap();
        assert_eq!(world.plants[0].energy, 0.25);
    }
}

#[test]
fn movement_uses_configured_speed_timestep_and_dead_zone() {
    let mut body = BodyParameters {
        max_speed: 12.0,
        movement_time_step: 0.25,
        ..BodyParameters::default()
    };
    let action = MoveAction {
        direction: DVec2::X,
        speed_fraction: 0.75,
    };
    assert_eq!(action.displacement(body), DVec2::new(2.25, 0.0));
    body.movement_dead_zone = 2.0;
    assert_eq!(action.displacement(body), DVec2::ZERO);
}

#[test]
fn sensing_uses_instance_world_scale_and_dead_zone() {
    let mut fixture = Fixture::debug(73);
    fixture.world.parameters.world.height = 40.0;
    fixture.world.plants[0].position = DVec2::new(8.0, 0.0);
    fixture.world.parameters.sensing = SensingConfig {
        distance_scale: 0.25,
        softening: 0.2,
        gradient_dead_zone: 10.0,
    };
    let sensed = fixture.world.sense(DVec2::ZERO);
    assert!((sensed.strength - 1.0 / 0.3).abs() < 1e-12);
    assert_eq!(sensed.gradient, DVec2::ZERO);
    fixture.world.parameters.sensing.gradient_dead_zone = 0.5;
    assert!(fixture.world.sense(DVec2::ZERO).gradient.distance(DVec2::X) < 1e-12);
}

#[test]
fn movement_cost_scales_with_energy_and_controlled_distance_after_wall_clamping() {
    for (start_x, direction, speed, distance) in [
        (0.0, DVec2::X, 0.0, 0.0),
        (0.0, DVec2::X, 0.5, 5.0),
        (0.0, DVec2::X, 1.0, 10.0),
        (0.0, DVec2::ZERO, 1.0, 0.0),
        (628.0, DVec2::X, 1.0, 2.0),
        (630.0, DVec2::X, 1.0, 0.0),
    ] {
        for (energy, metrics_enabled) in [0.0, 0.25, 0.5, 1.0]
            .into_iter()
            .flat_map(|energy| [false, true].map(|enabled| (energy, enabled)))
        {
            let mut config = Config::<(), (), FixedMutation> {
                initial_animals: 1,
                plants: 0,
                minimum_population: 0,
                ..Config::default()
            };
            config.immigration.position = Placement::Fixed(DVec2::new(start_x, 0.0));
            config.immigration.energy = energy;
            config.animal.body.upkeep = 0.0005;
            config.animal.body.movement_cost = 0.0009;
            config.animal.body.movement_energy_power = 1.0;
            config.animal.body.radius_scale = 0.0;
            config.metrics.enabled = metrics_enabled;
            let genome = FixedGenome {
                action: MoveAction {
                    direction,
                    speed_fraction: speed,
                },
                ..FixedGenome::default()
            };
            let mut world = Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(
                config,
                75,
                Some(genome),
            )
            .unwrap();
            let report = world.advance(1).unwrap();
            assert_eq!(report.decisions[0].energy, energy);
            assert_eq!(
                report.decisions[0].position_after - report.decisions[0].position_before,
                DVec2::new(distance, 0.0),
            );
            let expected = energy - 0.0005 - 0.0009 * energy * distance;
            assert!((world.animals[&id(1)].body.energy - expected).abs() < 1e-12);
        }
    }
}

#[test]
fn movement_cost_is_paid_before_birth_and_excludes_birth_offsets() {
    for energy in [1.0025, 1.0125] {
        let mut fixture = Fixture::debug(76);
        fixture.world.plants[0].position = DVec2::new(100.0, 100.0);
        fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = energy;
        fixture.world.parameters.body.upkeep = 0.0005;
        fixture.world.parameters.body.movement_energy_power = 1.0;
        fixture.world.parameters.body.movement_cost = 0.0009;
        fixture.action(MoveAction {
            direction: DVec2::X,
            speed_fraction: 1.0,
        });
        let report = fixture.tick();
        if energy == 1.0025 {
            assert!(
                report.events.is_empty(),
                "travel must be affordable before reproducing"
            );
            assert!((fixture.world.animals[&id(1)].body.energy - 0.9929775).abs() < 1e-12);
        } else {
            assert_eq!(
                report.events,
                [Event::Birth {
                    parent: id(1),
                    child: id(2)
                }]
            );
            assert_eq!(report.decisions.len(), 1);
            for animal in fixture.world.animals.values() {
                assert!((animal.body.energy - 0.50119375).abs() < 1e-12);
                assert_ne!(animal.body.position, report.decisions[0].position_after);
            }
        }
    }
}

#[test]
fn a_meal_does_not_retroactively_increase_the_movement_fee() {
    let mut fixture = Fixture::debug(79);
    fixture.world.parameters.body.upkeep = 0.0005;
    fixture.world.parameters.body.movement_energy_power = 1.0;
    fixture.world.parameters.body.movement_cost = 0.0009;
    fixture.action(MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    });
    let report = fixture.tick();
    assert_eq!(report.decisions[0].energy, 0.975);
    assert_eq!(report.decisions[0].position_after, DVec2::new(10.0, 0.0));
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
    // 0.975 + 0.1 food - 0.008775 travel, split equally before 0.0005 upkeep each.
    for animal in fixture.world.animals.values() {
        assert!((animal.body.energy - 0.5326125).abs() < 1e-12);
    }
}

#[test]
fn immigrants_pay_no_movement_cost_until_they_act() {
    let mut fixture = Fixture::debug(77);
    fixture.world.plants[0].position = DVec2::new(100.0, 100.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 0.00025;
    fixture.world.minimum_population = 1;
    fixture.world.parameters.body.upkeep = 0.0005;
    fixture.world.parameters.body.movement_energy_power = 1.0;
    fixture.world.parameters.body.movement_cost = 0.0009;
    fixture.action(MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    });
    fixture.tick();
    assert!((fixture.world.animals[&id(1)].body.energy + 0.00025225).abs() < 1e-12);
    let report = fixture.tick();
    assert!(report.decisions.is_empty());
    assert_eq!(
        report.events,
        [Event::Death { id: id(1) }, Event::Immigrant { id: id(2) }]
    );
    assert_eq!(fixture.world.animals[&id(2)].body.energy, 0.2495);
}

#[test]
fn movement_energy_power_supports_flat_fractional_and_nonlinear_costs() {
    for (energy, power, movement_cost, expected_fee) in [
        (0.0, 0.0, 0.00045, 0.0045),
        (0.0, 1.0, 0.0009, 0.0),
        (0.25, 0.0, 0.00045, 0.0045),
        (0.25, 0.5, 0.0009, 0.0045),
        (0.25, 1.0, 0.0009, 0.00225),
        (0.25, 2.0, 0.0009, 0.0005625),
    ] {
        let mut fixture = Fixture::debug(80);
        fixture.world.plants[0].position = DVec2::new(100.0, 100.0);
        fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = energy;
        fixture.world.parameters.body.upkeep = 0.0005;
        fixture.world.parameters.body.movement_cost = movement_cost;
        fixture.world.parameters.body.movement_energy_power = power;
        fixture.action(MoveAction {
            direction: DVec2::X,
            speed_fraction: 1.0,
        });
        fixture.tick();
        assert!(
            (fixture.world.animals[&id(1)].body.energy - (energy - expected_fee - 0.0005)).abs()
                < 1e-12
        );
    }
}

#[test]
fn changing_energy_units_preserves_geometry_and_tick_energy_budgets() {
    for (upkeep, cost, power) in [
        (0.005, 0.0, 0.0_f64),
        (0.0005, 0.00045, 0.0),
        (0.0005, 0.0009, 1.0),
        (0.0005, 0.0009, 0.5),
    ] {
        let mut config = Config::<(), (), FixedMutation> {
            initial_animals: 1,
            plants: 1,
            minimum_population: 0,
            ..Config::default()
        };
        config.immigration.energy = 0.975;
        config.immigration.position = Placement::Fixed(DVec2::ZERO);
        config.plant.initial_position = Placement::Fixed(DVec2::ZERO);
        config.animal.body.upkeep = upkeep;
        config.animal.body.movement_cost = cost;
        config.animal.body.movement_energy_power = power;
        let mut scaled = config.clone();
        scaled.immigration.energy *= 400.0;
        scaled.animal.reproduction.threshold *= 400.0;
        scaled.animal.body.upkeep *= 400.0;
        scaled.animal.body.movement_cost *= 400.0_f64.powf(1.0 - power);
        scaled.animal.body.radius_scale /= 20.0;
        scaled.plant.initial_energy *= 400.0;
        scaled.plant.replacement_energy *= 400.0;
        scaled.plant.max_energy *= 400.0;
        scaled.plant.growth *= 400.0;
        scaled.plant.radius_scale /= 20.0;
        let genome = FixedGenome {
            action: MoveAction {
                direction: DVec2::X,
                speed_fraction: 0.4,
            },
            ..FixedGenome::default()
        };
        let mut normalized = Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(
            config,
            81,
            Some(genome.clone()),
        )
        .unwrap();
        let mut other = Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(
            scaled,
            81,
            Some(genome),
        )
        .unwrap();
        for tick in 1..=32 {
            let report = normalized.advance(tick).unwrap();
            let scaled_report = other.advance(tick).unwrap();
            assert_eq!(report.events.len(), scaled_report.events.len());
            for (event, scaled_event) in report.events.iter().zip(&scaled_report.events) {
                match (event, scaled_event) {
                    (
                        Event::Meal { id, plant, gain },
                        Event::Meal {
                            id: other_id,
                            plant: other_plant,
                            gain: other_gain,
                        },
                    ) => {
                        assert_eq!((id, plant), (other_id, other_plant));
                        assert!((gain - other_gain / 400.0).abs() < 1e-12);
                    }
                    _ => assert_eq!(event, scaled_event),
                }
            }
            assert_eq!(normalized.animals.len(), other.animals.len());
            for (id, animal) in &normalized.animals {
                let scaled = &other.animals[id];
                assert!((animal.body.energy - scaled.body.energy / 400.0).abs() < 1e-12);
                assert!((animal.body.radius() - scaled.body.radius()).abs() < 1e-11);
                assert!(animal.body.position.distance(scaled.body.position) < 1e-11);
                assert_eq!(animal.mind.genome, scaled.mind.genome);
            }
            for (plant, scaled) in normalized.plants.iter().zip(&other.plants) {
                assert_eq!(plant.id, scaled.id);
                assert!((plant.energy - scaled.energy / 400.0).abs() < 1e-12);
                assert!((plant.radius() - scaled.radius()).abs() < 1e-11);
                assert!(plant.position.distance(scaled.position) < 1e-11);
            }
            assert_eq!(normalized.rng, other.rng);
            assert_eq!(normalized.context, other.context);
        }
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
        |c| c.world.height = f64::INFINITY,
        |c| c.immigration.energy = -0.0025,
        |c| c.immigration.position = Placement::Fixed(DVec2::splat(10000.0)),
        |c| c.animal.reproduction.threshold = 0.0,
        |c| c.animal.reproduction.child_energy_fraction = 1.0,
        |c| c.animal.reproduction.child_energy_fraction = 0.0,
        |c| c.animal.body.upkeep = -0.0025,
        |c| c.animal.body.movement_cost = -0.1,
        |c| c.animal.body.movement_cost = f64::NAN,
        |c| c.animal.body.movement_cost = f64::MAX,
        |c| c.animal.body.movement_energy_power = -1.0,
        |c| c.animal.body.movement_energy_power = f64::NAN,
        |c| c.animal.body.movement_energy_power = f64::INFINITY,
        |c| c.animal.body.max_speed = f64::INFINITY,
        |c| c.plant.max_energy = 0.0,
        |c| c.plant.initial_energy = 1.0025,
        |c| c.sensing.softening = 0.0,
        |c| c.genetics.initialization.hidden_units = 0,
        |c| c.genetics.mutation.max_delta = f32::NAN,
    ];
    for change in invalid_changes {
        let mut invalid = config.clone();
        change(&mut invalid);
        assert!(Ecosystem::new(invalid, 74).is_err());
    }
    let universe = Universe::new(config.clone(), 74).unwrap();
    for change in [
        (|c: &mut Config| c.world.width = 1000.0) as fn(&mut Config),
        |c| c.immigration.energy = 0.5,
        |c| c.plants = 1,
        |c| c.minimum_population = 1,
        |c| c.animal.body.movement_cost = 0.0009,
        |c| c.animal.body.movement_energy_power = 1.0,
        |c| c.genetics.initialization.hidden_units = 7,
        |c| c.genetics.mutation.max_delta = 0.1,
    ] {
        let mut changed = config.clone();
        change(&mut changed);
        assert!(universe.clone().restore(changed).is_err());
    }
    let mut world = Ecosystem::new(config, 74).unwrap();
    world.parameters.world.height = 0.0;
    assert!(world.validate().is_err());
}

#[test]
fn metrics_are_pure_and_do_not_change_reports_or_checkpoint_bytes() {
    use crate::metrics::Metrics;
    let mut config: Config = crate::test_support::ecosystem("debug");
    config.metrics.enabled = false;
    config.animal.body.movement_cost = 0.003;
    config.animal.body.movement_energy_power = 1.0;
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
    assert_eq!(first.food_per_tick, Some(0.1));
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
fn window_lengths_change_resolution_but_not_rate_units_or_totals() {
    use crate::metrics::Metrics;
    // Short and long windows include changing population, deaths and immigrants.
    // The partial final window must use its actual duration.
    let mut reference: Option<(Ecosystem, [f64; 5], u64)> = None;
    for window_ticks in [1, 7, 1000] {
        let mut config: Config = crate::test_support::ecosystem("baseline");
        config.metrics.window_ticks = window_ticks;
        config.animal.body.upkeep = 0.02;
        let mut world = Ecosystem::new(config, 17).unwrap();
        let mut totals = [0.0; 5];
        let mut exposure = 0;
        for tick in 1..=137 {
            world.advance(tick).unwrap();
            let window = world.metrics_window().unwrap();
            if window.complete || tick == 137 {
                let s = world.metrics();
                for (total, rate) in totals[..4].iter_mut().zip([
                    s.food_per_tick,
                    s.births_per_tick,
                    s.deaths_per_tick,
                    s.immigrants_per_tick,
                ]) {
                    *total += rate.unwrap() * window.samples as f64;
                }
                totals[4] +=
                    s.distance_per_animal_tick.unwrap_or(0.0) * window.active_animal_ticks as f64;
                exposure += window.active_animal_ticks;
            }
        }
        assert!(totals[2] > 0.0 && totals[3] > 0.0);
        if let Some((expected, expected_totals, expected_exposure)) = &reference {
            assert_eq!(&world, expected);
            assert_eq!(world.metrics().population, expected.metrics().population);
            assert_eq!(
                world.metrics().mean_animal_energy,
                expected.metrics().mean_animal_energy
            );
            assert_eq!(
                world.metrics().mean_plant_energy,
                expected.metrics().mean_plant_energy
            );
            for (a, b) in totals.iter().zip(expected_totals) {
                assert!((a - b).abs() < 1e-10 * a.abs().max(1.0), "{a} != {b}");
            }
            assert_eq!(exposure, *expected_exposure);
        } else {
            reference = Some((world, totals, exposure));
        }
    }
}

#[test]
fn metrics_restart_on_resume_and_discarded_updates_publish_nothing() {
    use crate::{metrics::Metrics, universe::Universe};
    let mut config: Config = crate::test_support::ecosystem("debug");
    config.metrics.window_ticks = 3;
    let mut original = Universe::new(config.clone(), 44).unwrap();
    original.advance().unwrap();
    original.advance().unwrap();
    let mut bytes = Vec::new();
    ciborium::into_writer(&original, &mut bytes).unwrap();
    let decoded: Universe = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(decoded.world().metrics(), Summary::default());
    let mut resumed = decoded.restore(config).unwrap();
    assert!(resumed.universe().read().world().metrics_window().is_none());
    assert_eq!(original.advance().unwrap(), resumed.tick().unwrap());
    assert_eq!(original, *resumed.universe().read());
    let window = resumed.universe().read().world().metrics_window().unwrap();
    assert_eq!(
        (window.start_tick, window.end_tick, window.samples),
        (2, 3, 1)
    );

    let mut fixture = Fixture::debug(45);
    let mut config = measured_config();
    fixture.world.configure(&config).unwrap();
    let before = fixture.world.metrics.clone();
    let (_, discarded) = fixture.world.compute_update(1).unwrap();
    drop(discarded);
    assert_eq!(fixture.world.metrics, before);
    fixture
        .world
        .animals
        .get_mut(&id(1))
        .unwrap()
        .mind
        .genome
        .fail_mind = true;
    assert!(fixture.world.compute_update(1).is_err());
    assert_eq!(fixture.world.metrics, before);
    config.metrics.window_ticks = 0;
    assert!(fixture.world.configure(&config).is_err());
    assert_eq!(fixture.world.metrics, before);
    config.metrics.enabled = false;
    fixture.world.configure(&config).unwrap();
    assert_eq!(fixture.world.metrics(), Summary::default());
}
