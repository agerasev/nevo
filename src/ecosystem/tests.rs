use super::*;

#[derive(Clone, Debug, Default, PartialEq)]
struct FixedMind {
    action: MoveAction,
    calls: usize,
    fail: bool,
}

impl Mind<GeneticContext> for FixedMind {
    type Input = FoodObservation;
    type Output = MoveAction;

    fn process(&mut self, _: &mut GeneticContext, _: FoodObservation) -> Result<MoveAction> {
        self.calls += 1;
        ensure!(!self.fail, "mind failed");
        Ok(self.action)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct FixedHeredity {
    fail_offspring: bool,
}

impl Heredity<GeneticContext, FixedMind> for FixedHeredity {
    fn immigrant(&self, _: &mut GeneticContext) -> Result<FixedMind> {
        Ok(FixedMind::default())
    }
    fn offspring(&self, _: &mut GeneticContext, parent: &FixedMind) -> Result<FixedMind> {
        ensure!(!self.fail_offspring, "offspring failed");
        Ok(FixedMind {
            action: parent.action,
            ..FixedMind::default()
        })
    }
}

struct Fixture {
    world: Ecosystem<FixedMind, FixedHeredity>,
    number: u64,
}

impl Fixture {
    fn debug(seed: u64) -> Self {
        let mut world = Ecosystem::empty(1, 0, seed, seed, FixedHeredity::default());
        world
            .admit(
                1,
                Admission {
                    position: Some(DVec2::ZERO),
                    energy: 390.0,
                },
            )
            .unwrap();
        world.seed_plants(Some(DVec2::ZERO)).unwrap();
        Self { world, number: 0 }
    }

    fn tick(&mut self) -> TickReport {
        self.number += 1;
        self.world.advance(self.number).unwrap()
    }

    fn action(&mut self, action: MoveAction) {
        for animal in self.world.animals.values_mut() {
            animal.mind.action = action;
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
        .admit(
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
    let mut world = Ecosystem::with_heredity(
        Scenario::Debug,
        7,
        HerbivoreHeredity::with_immigrant_genome(genome),
    )
    .unwrap();
    let before = world.clone();
    let sensed = world.sense(world.animals[&id(1)].body.position);
    assert_eq!(world, before, "sensing must not change the world");
    assert_eq!(sensed.strength, 100.0);

    let first = world.advance(1).unwrap();
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
        .admit(
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
fn failures_in_minds_actions_or_birth_leave_the_published_universe_unchanged() {
    use crate::universe::Universe;
    for phase in 0..3 {
        let mut fixture = Fixture::debug(11);
        match phase {
            0 => fixture.world.animals.get_mut(&id(1)).unwrap().mind.fail = true,
            1 => fixture.action(MoveAction {
                direction: DVec2::new(f64::INFINITY, 0.0),
                speed_fraction: 0.0,
            }),
            2 => fixture.world.heredity.fail_offspring = true,
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
    let mut initial = Ecosystem::empty(0, 0, 42, 17, HerbivoreHeredity::default());
    let arrivals = initial.admit(1, Admission::default()).unwrap();
    let mut later = Ecosystem::empty(0, 1, 42, 17, HerbivoreHeredity::default());
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
    assert!(fixture.world.admit(1, Admission::default()).is_err());
    assert_eq!(fixture.world.next_animal, u64::MAX);
    assert_eq!(fixture.world.animals.len(), 1);
}
