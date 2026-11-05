use super::*;
use crate::genome::{Genome, Mutate, RandomInit};

// Fixed reference values keep the historical default behavior checks explicit.
const HALF_SIZE: DVec2 = DVec2::new(630.0, 350.0);
const START_ENERGY: f64 = 100.0;
const RIPE_ENERGY: f64 = 400.0;
const UPKEEP: f64 = 2.0;

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

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
struct FixedMind<const FAIL_INIT: bool = false> {
    genome: FixedGenome<FAIL_INIT>,
    calls: usize,
    configurations: usize,
    metrics_enabled: bool,
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
            metrics_enabled: false,
        })
    }
}

impl<const FAIL_INIT: bool> Mind<GeneticContext> for FixedMind<FAIL_INIT> {
    type Input = FoodObservation;
    type Output = MoveAction;
    type Update = usize;
    type Config = ();

    fn configure(&mut self, _: &(), metrics_enabled: bool) -> Result<()> {
        self.configurations += 1;
        self.metrics_enabled = metrics_enabled;
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
                    energy: 390.0,
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
            gain: 40.0
        }]
    );
    assert_eq!(fixture.world.animals[&first].body.energy, 138.0);
    assert_eq!(fixture.world.animals[&second].body.energy, 98.0);
    assert_eq!(fixture.world.plants[0].energy, 1.0);
    assert_ne!(fixture.world.plants[0].id, 1);
}

#[test]
fn upkeep_defers_death_and_replacement_first_acts_on_the_following_tick() {
    let mut fixture = Fixture::debug(2);
    fixture.world.set_minimum_population(1);
    fixture.world.plants[0].position = DVec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 1.0;

    let starving = fixture.tick();
    assert!(starving.events.is_empty());
    assert_eq!(starving.decisions[0].energy, 1.0);
    assert_eq!(fixture.world.animals[&id(1)].body.energy, -1.0);
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
        (id(2), 98.0)
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
    assert_eq!(first.decisions[0].energy, 390.0);
    assert_eq!(first.decisions[0].observation, sensed);
    assert_eq!(world.animals[&id(1)].body.energy, 213.0);
    assert_eq!(world.animals[&id(2)].body.energy, 213.0);
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
                gain: 40.0
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
    assert_eq!(body.energy, 138.0);
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
                energy: -1.0,
            },
        )
        .unwrap();
    fixture.world.set_minimum_population(2);
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
        let original = universe.clone();
        assert!(universe.advance().is_err());
        assert_eq!(universe, original);
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
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 1700.0;
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
                energy: 500.0,
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
    assert_eq!(world.animals[&parent].body.energy, 248.0);
    assert_eq!(world.animals[&child].body.energy, 248.0);
}

#[test]
fn failed_random_initialization_does_not_publish_consumed_randomness() {
    use crate::universe::Universe;

    let world: Ecosystem<FixedMind<true>, FixedGenome<true>, FixedMutation> =
        Ecosystem::empty(0, 1, 11, 17, None, FixedMutation::default());
    let mut universe = Universe::from_world(world).unwrap();
    let original = universe.clone();
    assert!(
        universe
            .advance()
            .unwrap_err()
            .to_string()
            .contains("initialization failed")
    );
    assert_eq!(universe, original);
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
        energy: 100.0,
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
    let before = fixture.world.clone();
    let (first, update) = fixture.world.compute_update(1).unwrap();
    assert_eq!(fixture.world, before);
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
        let original = universe.clone();
        assert!(universe.advance().is_err());
        assert_eq!(universe, original);
    }
}

#[test]
fn stepping_requires_no_mind_clone_and_copies_genomes_only_for_arrivals_and_births() {
    use crate::{run::Run, universe::Universe};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct CountedGenome(Arc<AtomicUsize>);
    impl Clone for CountedGenome {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::Relaxed);
            Self(Arc::clone(&self.0))
        }
    }
    impl Genome<GeneticContext> for CountedGenome {}
    impl RandomInit<GeneticContext> for CountedGenome {
        type Config = ();
        fn random(_: &(), _: &mut GeneticContext) -> Result<Self> {
            panic!("this experiment uses an immigrant template")
        }
    }
    #[derive(Default, PartialEq)]
    struct Mutation;
    impl Clone for Mutation {
        fn clone(&self) -> Self {
            panic!("stepping must borrow mutation parameters")
        }
    }
    impl Mutate<GeneticContext, Mutation> for CountedGenome {
        fn mutate(&mut self, _: &Mutation, _: &mut GeneticContext) -> Result<()> {
            Ok(())
        }
    }
    // Deliberately not Clone: persistent brains can own expensive backend state.
    struct CountingMind {
        genome: CountedGenome,
        calls: usize,
    }
    impl From<CountedGenome> for CountingMind {
        fn from(genome: CountedGenome) -> Self {
            Self { genome, calls: 0 }
        }
    }
    impl AsRef<CountedGenome> for CountingMind {
        fn as_ref(&self) -> &CountedGenome {
            &self.genome
        }
    }
    impl Mind<GeneticContext> for CountingMind {
        type Input = FoodObservation;
        type Output = MoveAction;
        type Update = usize;
        type Config = ();
        fn compute_update(
            &self,
            _: FoodObservation,
            _: &mut GeneticContext,
        ) -> Result<(MoveAction, usize)> {
            Ok((MoveAction::default(), self.calls + 1))
        }
        fn apply_update(&mut self, calls: usize) {
            self.calls = calls;
        }
    }

    let copies = Arc::new(AtomicUsize::new(0));
    let mut world: Ecosystem<CountingMind, CountedGenome, Mutation> = Ecosystem::empty(
        0,
        0,
        1,
        42,
        Some(CountedGenome(Arc::clone(&copies))),
        Mutation,
    );
    let animal = world
        .admit_with(
            1,
            Admission {
                position: Some(DVec2::ZERO),
                energy: 100.0,
            },
        )
        .unwrap()[0];
    assert_eq!(copies.load(Ordering::Relaxed), 1, "one immigrant genome");
    let mut run = Run::from_universe(Universe::from_world(world).unwrap());
    // Initial validation also checks the template by developing a fresh mind.
    let before = copies.load(Ordering::Relaxed);
    for _ in 0..3 {
        assert!(run.tick().unwrap().events.is_empty());
    }
    assert_eq!(
        copies.load(Ordering::Relaxed),
        before,
        "ordinary ticks never copy a genome"
    );
    assert_eq!(
        run.universe().read().world().animals()[&animal].mind.calls,
        3
    );

    let mut world: Ecosystem<CountingMind, CountedGenome, Mutation> = Ecosystem::empty(
        0,
        0,
        1,
        42,
        Some(CountedGenome(Arc::clone(&copies))),
        Mutation,
    );
    world
        .admit_with(
            1,
            Admission {
                position: Some(DVec2::ZERO),
                energy: 1700.0,
            },
        )
        .unwrap();
    let mut run = Run::from_universe(Universe::from_world(world).unwrap());
    let before = copies.load(Ordering::Relaxed);
    let report = run.tick().unwrap();
    assert_eq!(
        report
            .events
            .iter()
            .filter(|event| matches!(event, Event::Birth { .. }))
            .count(),
        3
    );
    assert_eq!(
        copies.load(Ordering::Relaxed) - before,
        3,
        "one genome per birth, including newborn parents"
    );
}

impl<const FAIL_INIT: bool> crate::metrics::Metrics for FixedGenome<FAIL_INIT> {
    type Summary = ();
    fn collect_metrics(&self, _: &mut ()) {}
}
impl<const FAIL_INIT: bool> crate::metrics::Metrics for FixedMind<FAIL_INIT> {
    type Summary = ();
    fn collect_metrics(&self, _: &mut ()) {}
}

fn measured_config<MC: Default>() -> Config<MC, (), FixedMutation> {
    use crate::metrics::Selection;
    let mut config = Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        metrics: MetricsConfig {
            enabled: true,
            window_ticks: 3,
            population: true,
            event_rates: true,
            genetic_diversity: true,
        },
        ..Config::default()
    };
    config.animal.metrics = AnimalMetricsConfig {
        food_gained: Selection::all(),
        offspring: Selection::all(),
        distance: Selection::all(),
        observed_survival: Selection::all(),
    };
    config.animal.body.metrics.energy = Selection::all();
    config.plant.metrics.energy = Selection::all();
    config
}

#[test]
fn metrics_selection_preserves_every_report_and_checkpoint_byte() {
    use crate::metrics::{Metrics, Selection};
    let mut config: Config = crate::test_support::ecosystem("debug");
    config.metrics.enabled = false;
    let mut selected = config.clone();
    selected.metrics = measured_config::<()>().metrics;
    selected.animal.metrics = measured_config::<()>().animal.metrics;
    selected.animal.body.metrics.energy = Selection::all();
    selected.animal.mind.brain.metrics.activation = Selection::all();
    selected.animal.mind.brain.metrics.saturation = true;
    selected.plant.metrics.energy = Selection::all();
    let mut plain = Ecosystem::new(config.clone(), 42).unwrap();
    let mut measured = Ecosystem::new(selected.clone(), 42).unwrap();
    for tick in 1..=120 {
        assert_eq!(
            plain.advance(tick).unwrap(),
            measured.advance(tick).unwrap()
        );
        let encode = |world: &Ecosystem| {
            let mut bytes = Vec::new();
            ciborium::into_writer(world, &mut bytes).unwrap();
            bytes
        };
        assert_eq!(encode(&plain), encode(&measured), "tick {tick}");
        assert_eq!(plain, measured, "caches are not simulation state");
        let summary = measured.metrics();
        if tick == 1 {
            assert_eq!(
                summary.animal.mind.value().activation.unwrap().count,
                Some(4),
                "mind receives its metrics config; newborn has not acted"
            );
        }
        assert_eq!(
            summary.animal.body.energy.value().count,
            Some(measured.animals.len() as u64)
        );
        assert_eq!(
            summary.genome.unwrap().value().genomes,
            measured.animals.len() as u64
        );
        assert!(plain.metrics().window.is_none());
    }
}

#[test]
fn window_reports_use_actual_samples_and_active_animal_exposure() {
    use crate::metrics::Metrics;
    let mut fixture = Fixture::debug(43);
    let config = measured_config();
    fixture.world.configure(&config).unwrap();
    fixture.world.advance(1).unwrap();
    let first = fixture.world.metrics();
    assert_eq!(
        first.window,
        Some(Window {
            start_tick: 0,
            end_tick: 1,
            samples: 1,
            complete: false
        })
    );
    let events = first.events.unwrap();
    assert_eq!(
        (events.meals, events.births, events.active_animal_ticks),
        (1, 1, 1)
    );
    assert_eq!(events.births_per_tick, 1.0);
    assert_eq!(events.food_per_animal_tick, Some(40.0));
    assert_eq!(first.animal.body.food_gained.value().sum, Some(40.0));
    assert_eq!(first.animal.body.offspring.value().mean, Some(0.5));
    assert_eq!(first.observed_survival.unwrap().mean, None);
    fixture.world.advance(2).unwrap();
    let second = fixture.world.metrics();
    let events = second.events.unwrap();
    assert_eq!(
        events.active_animal_ticks, 3,
        "newborn first acts next tick"
    );
    assert_eq!(events.births_per_tick, 0.5);
    assert_eq!(events.births_per_animal_tick, Some(1.0 / 3.0));
    fixture.world.advance(3).unwrap();
    assert!(fixture.world.metrics_window().unwrap().complete);
    fixture.world.advance(4).unwrap();
    let reset = fixture.world.metrics();
    assert_eq!(
        reset.window,
        Some(Window {
            start_tick: 3,
            end_tick: 4,
            samples: 1,
            complete: false
        })
    );
    assert_eq!(reset.events.unwrap().births, 0);
    assert_eq!(reset.animal.body.offspring.value().sum, Some(0.0));
}

#[test]
fn cbor_resume_starts_fresh_metrics_at_the_restored_tick() {
    use crate::{metrics::Metrics, universe::Universe};
    let mut selected: Config = crate::test_support::ecosystem("debug");
    selected.metrics = measured_config::<()>().metrics;
    selected.animal.metrics = measured_config::<()>().animal.metrics;
    selected.animal.body.metrics.energy = crate::metrics::Selection::all();
    let mut original = Universe::new(selected.clone(), 44).unwrap();
    original.advance().unwrap();
    original.advance().unwrap();
    let mut bytes = Vec::new();
    ciborium::into_writer(&original, &mut bytes).unwrap();
    let decoded: Universe = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(original, decoded);
    assert!(decoded.world().metrics_window().is_none());
    assert!(!decoded.world().metrics_enabled());
    let mut resumed = decoded.restore(selected).unwrap();
    {
        let state = resumed.universe().read();
        assert!(state.world().metrics_enabled());
        assert!(state.world().metrics_window().is_none());
        let initial = state.world().metrics();
        assert!(initial.events.is_none());
        assert!(initial.population.is_some());
    }
    assert_eq!(original.advance().unwrap(), resumed.tick().unwrap());
    let state = resumed.universe().read();
    assert_eq!(original, *state);
    let summary = state.world().metrics();
    assert_eq!(
        summary.window,
        Some(Window {
            start_tick: 2,
            end_tick: 3,
            samples: 1,
            complete: false,
        })
    );
    assert_eq!(summary.events.unwrap().births, 0);
    assert_eq!(summary.animal.body.offspring.value().sum, Some(0.0));
}

#[test]
fn failed_or_discarded_computation_does_not_publish_metrics() {
    let mut fixture = Fixture::debug(45);
    let config = measured_config();
    fixture.world.configure(&config).unwrap();
    let before = fixture.world.metrics.clone();
    let body_before = fixture.world.animals[&id(1)].body.metrics;
    let (_, discarded) = fixture.world.compute_update(1).unwrap();
    drop(discarded);
    assert_eq!(fixture.world.metrics, before);
    assert_eq!(fixture.world.animals[&id(1)].body.metrics, body_before);
    fixture.world.advance(1).unwrap();
    let before = fixture.world.metrics.clone();
    let body_metrics = fixture.world.animals[&id(1)].body.metrics;
    fixture
        .world
        .animals
        .get_mut(&id(1))
        .unwrap()
        .mind
        .genome
        .fail_mind = true;
    assert!(fixture.world.compute_update(2).is_err());
    assert_eq!(fixture.world.metrics, before);
    assert_eq!(fixture.world.animals[&id(1)].body.metrics, body_metrics);
}

#[test]
fn deaths_report_window_observation_and_empty_exposure_is_unavailable() {
    use crate::metrics::Metrics;
    let mut fixture = Fixture::debug(46);
    fixture.world.plants[0].position.x = 100.0;
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 1.0;
    let config = measured_config();
    fixture.world.configure(&config).unwrap();
    fixture.world.advance(1).unwrap();
    fixture.world.advance(2).unwrap();
    let summary = fixture.world.metrics();
    assert_eq!(summary.observed_survival.unwrap().mean, Some(1.0));
    let events = summary.events.unwrap();
    assert_eq!((events.deaths, events.active_animal_ticks), (1, 1));
    assert_eq!(events.deaths_per_tick, 0.5);
    assert_eq!(summary.animal.body.energy.value().count, Some(0));
    assert_eq!(summary.animal.body.energy.value().mean, None);
    fixture.world.advance(3).unwrap();
    fixture.world.advance(4).unwrap();
    let summary = fixture.world.metrics();
    assert_eq!(summary.events.unwrap().food_per_animal_tick, None);
    assert_eq!(summary.observed_survival.unwrap().count, Some(0));
}

#[test]
fn explicit_configuration_validates_and_starts_a_clean_window() {
    use crate::metrics::{Metrics, Selection};
    let mut fixture = Fixture::debug(47);
    let mut config = measured_config();
    fixture.world.configure(&config).unwrap();
    let mut invalid = config.clone();
    invalid.metrics.window_ticks = 0;
    assert!(invalid.validate().is_err());
    let before = fixture.world.metrics.clone();
    assert!(fixture.world.configure(&invalid).is_err());
    assert_eq!(fixture.world.metrics, before);
    fixture.world.advance(1).unwrap();
    config.animal.body.metrics.energy = Selection {
        min: true,
        ..Selection::default()
    };
    config.animal.metrics.food_gained = Selection::default();
    fixture.world.configure(&config).unwrap();
    fixture.world.advance(2).unwrap();
    let summary = fixture.world.metrics();
    assert_eq!(summary.window.unwrap().start_tick, 1);
    assert!(!summary.animal.body.food_gained.enabled());
    let energy = summary.animal.body.energy.value();
    assert_eq!(energy.mean, None);
    assert!(energy.min.is_some());
    assert!(config.same_model(&invalid));
    invalid.initial_animals += 1;
    assert!(!config.same_model(&invalid));
}

#[test]
fn ecosystem_collection_replaces_one_snapshot_including_when_disabled() {
    use crate::metrics::Metrics;
    let mut fixture = Fixture::debug(48);
    let mut config = measured_config();
    fixture.world.configure(&config).unwrap();
    fixture.world.advance(1).unwrap();
    let mut summary = fixture.world.metrics();
    let first = serde_json::to_value(&summary).unwrap();
    fixture.world.collect_metrics(&mut summary);
    assert_eq!(serde_json::to_value(&summary).unwrap(), first);
    config.metrics.enabled = false;
    fixture.world.configure(&config).unwrap();
    fixture.world.advance(2).unwrap();
    fixture.world.collect_metrics(&mut summary);
    assert!(summary.window.is_none());
    assert!(summary.events.is_none());
    assert!(summary.population.is_none());
    assert!(summary.genome.is_none());
    assert!(!summary.animal.body.energy.enabled());
}

#[test]
fn lightweight_defaults_apply_before_the_first_tick_and_allow_field_overrides() {
    use crate::metrics::Metrics;
    let config = Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        ..Config::default()
    };
    let mut world = Ecosystem::new(config, 49).unwrap();
    assert!(world.metrics_enabled());
    assert!(world.metrics_window().is_none());
    let initial = world.metrics();
    assert_eq!(
        initial.population.unwrap(),
        PopulationSummary {
            animals: 1,
            plants: 1
        }
    );
    assert!(initial.events.is_none());
    assert!(initial.genome.is_none());
    assert_eq!(initial.animal.body.energy.value().mean, Some(START_ENERGY));
    assert_eq!(initial.plant.energy.value().min, Some(RIPE_ENERGY));
    assert!(!initial.animal.body.distance.enabled());
    assert!(initial.animal.mind.value().activation.is_none());
    world.advance(1).unwrap();
    assert!(world.metrics().events.is_some());

    let partial: Config = toml::from_str(
        r#"
[animal.body.metrics.energy]
mean = false
[plant.metrics.energy]
mean = false
"#,
    )
    .unwrap();
    for selection in [
        partial.animal.body.metrics.energy,
        partial.plant.metrics.energy,
    ] {
        assert!(!selection.mean);
        assert!(selection.min && selection.max);
        assert!(!selection.count && !selection.sum && !selection.variance);
    }
    let off: Config = toml::from_str(
        r#"
[animal.body.metrics.energy]
mean = false
min = false
max = false
[plant.metrics.energy]
mean = false
min = false
max = false
"#,
    )
    .unwrap();
    assert!(!off.animal.body.metrics.energy.enabled());
    assert!(!off.plant.metrics.energy.enabled());
}

#[test]
fn setup_occurs_once_for_initial_animals_births_and_immigrants() {
    use crate::metrics::Metrics;
    let mut config: Config<(), (), FixedMutation> = measured_config();
    config.initial_animals = 1;
    config.immigration.energy = 390.0;
    config.immigration.position = Placement::Fixed(DVec2::ZERO);
    config.plant.initial_position = Placement::Fixed(DVec2::ZERO);
    config.plants = 1;
    config.minimum_population = 0;
    config.metrics.window_ticks = 1;
    let mut world =
        Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(config, 50, None)
            .unwrap();
    assert_eq!(world.animals[&id(1)].mind.configurations, 1);
    assert!(world.animals[&id(1)].mind.metrics_enabled);
    world.advance(1).unwrap();
    assert_eq!(world.animals.len(), 2);
    assert_eq!(world.metrics().animal.body.offspring.value().sum, Some(1.0));
    assert_eq!(world.animals[&id(2)].body.metrics.offspring, 0);
    world.advance(2).unwrap();
    assert_eq!(world.metrics().animal.body.offspring.value().sum, Some(0.0));
    for animal in world.animals.values() {
        assert_eq!(animal.mind.configurations, 1);
        assert!(animal.body.metrics.body.energy.enabled());
        assert!(animal.mind.metrics_enabled);
    }
    world.set_minimum_population(3);
    world.advance(3).unwrap();
    let immigrant = &world.animals[&id(3)];
    assert_eq!(immigrant.mind.configurations, 1);
    assert_eq!(immigrant.mind.calls, 0);
    assert!(immigrant.mind.metrics_enabled);
    assert!(immigrant.body.metrics.body.energy.enabled());
    assert!(
        world
            .plants
            .iter()
            .all(|plant| plant.metrics.energy.enabled())
    );
}

#[test]
fn master_disable_reaches_every_collector_on_creation_resume_and_new_arrivals() {
    use crate::{
        metrics::{Metrics, Selection},
        universe::Universe,
    };
    let mut config: Config = crate::test_support::ecosystem("debug");
    config.metrics = measured_config::<()>().metrics;
    config.animal.metrics = measured_config::<()>().animal.metrics;
    config.animal.body.metrics.energy = Selection::all();
    config.plant.metrics.energy = Selection::all();
    config.animal.mind.brain.metrics.activation = Selection::all();
    config.animal.mind.brain.metrics.saturation = true;
    let mut measured = Universe::new(config.clone(), 51).unwrap();
    measured.advance().unwrap();
    assert!(
        measured
            .world()
            .metrics()
            .animal
            .mind
            .value()
            .activation
            .is_some()
    );
    let mut bytes = Vec::new();
    ciborium::into_writer(&measured, &mut bytes).unwrap();
    config.metrics.enabled = false;
    config.metrics.window_ticks = 0; // Disabled collectors need no valid window.
    let decoded: Universe = ciborium::from_reader(bytes.as_slice()).unwrap();
    let mut resumed = decoded.restore(config.clone()).unwrap();
    for _ in 0..3 {
        let state = resumed.universe().read();
        assert!(!state.world().metrics_enabled());
        assert!(state.world().metrics_window().is_none());
        for animal in state.world().animals.values() {
            assert_eq!(animal.body.metrics, metrics::BodyCache::default());
            let brain = animal.mind.metrics().value();
            assert!(brain.activation.is_none());
            assert!(brain.saturation_fraction.is_none());
        }
        assert!(
            state
                .world()
                .plants
                .iter()
                .all(|plant| !plant.metrics.energy.enabled())
        );
        drop(state);
        resumed.tick().unwrap();
    }
    let mut disabled = Ecosystem::new(config, 51).unwrap();
    disabled.advance(1).unwrap(); // Feeding creates a newborn under disabled settings.
    assert_eq!(disabled.animals.len(), 2);
    disabled.set_minimum_population(3);
    disabled.advance(2).unwrap(); // Immigration uses the same disabled policy.
    disabled.advance(3).unwrap(); // All new minds have now acted.
    for animal in disabled.animals.values() {
        assert_eq!(animal.body.metrics, metrics::BodyCache::default());
        assert!(animal.mind.metrics().value().activation.is_none());
        assert!(animal.mind.metrics().value().saturation_fraction.is_none());
    }
    assert!(
        disabled
            .plants
            .iter()
            .all(|plant| !plant.metrics.energy.enabled())
    );
    assert!(disabled.metrics().window.is_none());
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
        fn configure(&mut self, config: &u8, _: bool) -> Result<()> {
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
            energy: 250.0,
            position: Placement::Fixed(DVec2::new(7.0, -9.0)),
        },
        ..Config::default()
    };
    config.animal.body.upkeep = 3.0;
    config.animal.body.radius_scale = 0.2;
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
    assert_eq!(first.body.energy - 3.0, next.body.energy);
    assert_eq!(first.body.radius_scale, next.body.radius_scale);
    assert_eq!(first.mind, next.mind);
    assert_eq!(next.mind.brain.memory.len(), 7);
    assert_eq!(initial.rng, later.rng);
    assert_eq!(initial.context, later.context);
}

#[test]
fn configured_physics_controls_feeding_births_growth_and_body_size() {
    let mut config = Config::<(), (), FixedMutation> {
        initial_animals: 1,
        minimum_population: 0,
        plants: 1,
        world: WorldConfig {
            width: 80.0,
            height: 40.0,
        },
        immigration: ImmigrationConfig {
            energy: 30.0,
            position: Placement::Fixed(DVec2::ZERO),
        },
        ..Config::default()
    };
    config.animal.body.upkeep = 1.5;
    config.animal.body.radius_scale = 1.0;
    config.animal.reproduction = ReproductionConfig {
        threshold: 35.0,
        child_energy_fraction: 0.25,
        separation_scale: 0.0,
    };
    config.animal.feeding = FeedingConfig {
        efficiency: 0.5,
        contact_scale: 1.0,
    };
    config.plant.initial_position = Placement::Fixed(DVec2::ZERO);
    config.plant.replacement_position = Placement::Fixed(DVec2::new(30.0, 0.0));
    config.plant.initial_energy = 20.0;
    config.plant.replacement_energy = 5.0;
    config.plant.max_energy = 50.0;
    config.plant.growth = 3.5;
    config.plant.radius_scale = 2.0;
    let mut world =
        Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(config, 72, None)
            .unwrap();
    let report = world.advance(1).unwrap();
    assert_eq!(
        report.events,
        [
            Event::Meal {
                id: id(1),
                plant: 1,
                gain: 10.0
            },
            Event::Birth {
                parent: id(1),
                child: id(2)
            },
        ]
    );
    assert_eq!(world.animals[&id(1)].body.energy, 28.5);
    assert_eq!(world.animals[&id(2)].body.energy, 8.5);
    for animal in world.animals.values() {
        assert_eq!(animal.body.position, DVec2::ZERO);
        assert_eq!(animal.body.radius(), animal.body.energy.sqrt());
    }
    assert_eq!(world.plants[0].energy, 8.5);
    assert_eq!(world.plants[0].position, DVec2::new(30.0, 0.0));
    assert_eq!(world.plants[0].radius(), 2.0 * 8.5_f64.sqrt());
    for tick in 2..=30 {
        world.advance(tick).unwrap();
    }
    assert_eq!(world.plants[0].energy, 50.0);
    assert_eq!(world.world_config().half_size(), DVec2::new(40.0, 20.0));
    assert!(world.plants[0].position.x + world.plants[0].radius() <= 40.0);
}

#[test]
fn configured_movement_and_sensing_use_instance_world_rules() {
    let mut config = Config::<(), (), FixedMutation> {
        initial_animals: 1,
        minimum_population: 0,
        plants: 1,
        world: WorldConfig {
            width: 40.0,
            height: 40.0,
        },
        immigration: ImmigrationConfig {
            energy: 100.0,
            position: Placement::Fixed(DVec2::ZERO),
        },
        ..Config::default()
    };
    config.animal.body.upkeep = 0.0;
    config.animal.body.max_speed = 12.0;
    config.animal.body.movement_time_step = 0.25;
    config.animal.body.radius_scale = 0.1;
    config.animal.feeding.contact_scale = 0.0;
    config.plant.initial_position = Placement::Fixed(DVec2::new(8.0, 0.0));
    config.plant.initial_energy = 20.0;
    config.plant.max_energy = 50.0;
    config.sensing = SensingConfig {
        distance_scale: 0.25,
        softening: 0.2,
        gradient_dead_zone: 10.0,
    };
    let genome = FixedGenome {
        action: MoveAction {
            direction: DVec2::X,
            speed_fraction: 0.75,
        },
        ..FixedGenome::default()
    };
    let mut world = Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(
        config.clone(),
        73,
        Some(genome.clone()),
    )
    .unwrap();
    let sensed = world.sense(DVec2::ZERO);
    assert!((sensed.strength - 0.4 / 0.3).abs() < 1e-12);
    assert_eq!(sensed.gradient, DVec2::ZERO);
    world.advance(1).unwrap();
    assert_eq!(world.animals[&id(1)].body.position, DVec2::new(2.25, 0.0));
    for tick in 2..=12 {
        world.advance(tick).unwrap();
    }
    assert_eq!(world.animals[&id(1)].body.position, DVec2::new(19.0, 0.0));

    config.animal.body.movement_dead_zone = 2.0;
    config.sensing.gradient_dead_zone = 0.5;
    let mut world =
        Ecosystem::<FixedMind, FixedGenome, FixedMutation>::with_genetics(config, 73, Some(genome))
            .unwrap();
    assert_eq!(world.sense(DVec2::ZERO).gradient, DVec2::X);
    world.advance(1).unwrap();
    assert_eq!(world.animals[&id(1)].body.position, DVec2::ZERO);
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
        for (energy, metrics_enabled) in [0.0, 100.0, 200.0, 400.0]
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
            config.animal.body.upkeep = 0.2;
            config.animal.body.movement_cost_per_energy_distance = 0.0009;
            config.animal.body.radius_scale = 0.0;
            config.metrics.enabled = metrics_enabled;
            config.animal.metrics.distance = crate::metrics::Selection::all();
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
            let expected = energy - 0.2 - 0.0009 * energy * distance;
            assert!((world.animals[&id(1)].body.energy - expected).abs() < 1e-12);
        }
    }
}

#[test]
fn movement_cost_is_paid_before_birth_and_excludes_birth_offsets() {
    for energy in [401.0, 405.0] {
        let mut fixture = Fixture::debug(76);
        fixture.world.plants[0].position = DVec2::new(100.0, 100.0);
        fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = energy;
        fixture.world.parameters.body.upkeep = 0.2;
        fixture
            .world
            .parameters
            .body
            .movement_cost_per_energy_distance = 0.0009;
        fixture.action(MoveAction {
            direction: DVec2::X,
            speed_fraction: 1.0,
        });
        let report = fixture.tick();
        if energy == 401.0 {
            assert!(
                report.events.is_empty(),
                "travel must be affordable before reproducing"
            );
            assert!((fixture.world.animals[&id(1)].body.energy - 397.191).abs() < 1e-12);
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
                assert!((animal.body.energy - 200.4775).abs() < 1e-12);
                assert_ne!(animal.body.position, report.decisions[0].position_after);
            }
        }
    }
}

#[test]
fn a_meal_does_not_retroactively_increase_the_movement_fee() {
    let mut fixture = Fixture::debug(79);
    fixture.world.parameters.body.upkeep = 0.2;
    fixture
        .world
        .parameters
        .body
        .movement_cost_per_energy_distance = 0.0009;
    fixture.action(MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    });
    let report = fixture.tick();
    assert_eq!(report.decisions[0].energy, 390.0);
    assert_eq!(report.decisions[0].position_after, DVec2::new(10.0, 0.0));
    assert_eq!(
        report.events,
        [
            Event::Meal {
                id: id(1),
                plant: 1,
                gain: 40.0
            },
            Event::Birth {
                parent: id(1),
                child: id(2)
            },
        ]
    );
    // 390 + 40 food - 3.51 travel = 426.49, split equally before 0.2 upkeep each.
    for animal in fixture.world.animals.values() {
        assert!((animal.body.energy - 213.045).abs() < 1e-12);
    }
}

#[test]
fn immigrants_pay_no_movement_cost_until_they_act() {
    let mut fixture = Fixture::debug(77);
    fixture.world.plants[0].position = DVec2::new(100.0, 100.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().body.energy = 0.1;
    fixture.world.set_minimum_population(1);
    fixture.world.parameters.body.upkeep = 0.2;
    fixture
        .world
        .parameters
        .body
        .movement_cost_per_energy_distance = 0.0009;
    fixture.action(MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    });
    fixture.tick();
    assert!((fixture.world.animals[&id(1)].body.energy + 0.1009).abs() < 1e-12);
    let report = fixture.tick();
    assert!(report.decisions.is_empty());
    assert_eq!(
        report.events,
        [Event::Death { id: id(1) }, Event::Immigrant { id: id(2) }]
    );
    assert_eq!(fixture.world.animals[&id(2)].body.energy, 99.8);
}

#[test]
fn movement_cost_trajectories_are_independent_of_metric_selection() {
    let mut config: Config = crate::test_support::ecosystem("baseline");
    config.animal.body.upkeep = 0.2;
    config.animal.body.movement_cost_per_energy_distance = 0.0009;
    config.animal.metrics.distance = crate::metrics::Selection::all();
    let mut measured = Ecosystem::new(config.clone(), 78).unwrap();
    config.metrics.enabled = false;
    let mut unmeasured = Ecosystem::new(config, 78).unwrap();
    for tick in 1..=256 {
        assert_eq!(
            measured.advance(tick).unwrap(),
            unmeasured.advance(tick).unwrap()
        );
    }
    assert_eq!(measured, unmeasured);
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
        |c| c.immigration.energy = -1.0,
        |c| c.immigration.position = Placement::Fixed(DVec2::splat(10000.0)),
        |c| c.animal.reproduction.threshold = 0.0,
        |c| c.animal.reproduction.child_energy_fraction = 1.0,
        |c| c.animal.reproduction.child_energy_fraction = 0.0,
        |c| c.animal.body.upkeep = -1.0,
        |c| c.animal.body.movement_cost_per_energy_distance = -0.1,
        |c| c.animal.body.movement_cost_per_energy_distance = f64::NAN,
        |c| c.animal.body.movement_cost_per_energy_distance = f64::MAX,
        |c| c.animal.body.max_speed = f64::INFINITY,
        |c| c.plant.max_energy = 0.0,
        |c| c.plant.initial_energy = 401.0,
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
        |c| c.immigration.energy = 200.0,
        |c| c.plants = 1,
        |c| c.minimum_population = 1,
        |c| c.animal.body.movement_cost_per_energy_distance = 0.0009,
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
