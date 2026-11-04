use super::*;

struct Fixture {
    world: Ecosystem,
    handles: HandleAllocator,
    number: u64,
    floor: usize,
    action: MoveAction,
    changes: Vec<Lifecycle<Origin>>,
}

impl Fixture {
    fn debug(seed: u64) -> Self {
        let mut world = Ecosystem::empty(1, seed);
        let mut handles = HandleAllocator::new();
        let initial = world
            .admit(
                1,
                Admission {
                    position: Some(DVec2::ZERO),
                    energy: 390.0,
                },
                &mut handles,
            )
            .unwrap();
        world.seed_plants(Some(DVec2::ZERO)).unwrap();
        let PopulationObservation::Lifecycle { changes, .. } = initial else {
            panic!()
        };
        Self {
            world,
            handles,
            number: 0,
            floor: 0,
            action: MoveAction::default(),
            changes,
        }
    }

    fn tick(&mut self) -> TickReport {
        let PopulationObservation::Inputs { inputs, .. } = self.world.observe() else {
            panic!()
        };
        let actions = inputs
            .into_iter()
            .map(|(id, _)| (id, self.action))
            .collect();
        self.number += 1;
        let (report, observation) = self
            .world
            .advance(self.number, actions, &mut self.handles, |request| {
                Ok(Immigration {
                    count: self.floor.saturating_sub(request.population),
                })
            })
            .unwrap();
        let PopulationObservation::Lifecycle { changes, .. } = observation else {
            panic!()
        };
        self.changes.extend(changes);
        report
    }
}

fn id(serial: u64) -> Handle {
    let mut allocator = HandleAllocator::new();
    (0..serial)
        .map(|_| allocator.allocate().unwrap())
        .last()
        .unwrap()
}

#[test]
fn first_claimant_gets_the_only_meal_and_replacement_starts_small() {
    let mut fixture = Fixture::debug(1);
    let first = id(1);
    fixture.world.animals.get_mut(&first).unwrap().energy = START_ENERGY;
    let second = fixture
        .world
        .spawn(
            &mut fixture.handles,
            Origin::Immigrant,
            DVec2::ZERO,
            START_ENERGY,
            &mut fixture.changes,
        )
        .unwrap();

    let tick = fixture.tick();
    assert_eq!(
        tick.events,
        [Event::Meal {
            id: first,
            plant: 1,
            gain: 40.0
        }]
    );
    assert_eq!(fixture.world.animals[&first].energy, 138.0);
    assert_eq!(fixture.world.animals[&second].energy, 98.0);
    assert_eq!(fixture.world.plants[0].energy, 1.0);
    assert_ne!(fixture.world.plants[0].id, 1);
}

#[test]
fn upkeep_defers_death_and_replacement_first_acts_on_the_following_tick() {
    let mut fixture = Fixture::debug(2);
    fixture.floor = 1;
    fixture.world.plants[0].position = DVec2::new(100.0, 0.0);
    fixture.world.animals.get_mut(&id(1)).unwrap().energy = 1.0;

    let starving = fixture.tick();
    assert!(starving.events.is_empty());
    assert_eq!(starving.decisions[0].energy, 1.0);
    assert_eq!(fixture.world.animals[&id(1)].energy, -1.0);
    assert!(starving.decisions[0].position_after.is_finite());

    fixture.world.plants[0].position = starving.decisions[0].position_after;
    let replacement = fixture.tick();
    assert_eq!(
        replacement.events,
        [Event::Death { id: id(1) }, Event::Immigrant { id: id(2) }]
    );
    assert_eq!(
        fixture.changes[1..],
        [
            Lifecycle::Despawn { handle: id(1) },
            Lifecycle::Spawn {
                handle: id(2),
                origin: Origin::Immigrant
            },
        ]
    );
    assert!(replacement.decisions.is_empty());
    let next = fixture.tick();
    assert_eq!(next.decisions.len(), 1);
    assert_eq!(
        (next.decisions[0].id, next.decisions[0].energy),
        (id(2), 98.0)
    );
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
    fixture.action = MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    };
    let body = fixture.world.animals.get_mut(&id(1)).unwrap();
    body.position.x = HALF_SIZE.x - 5.0;
    body.energy = START_ENERGY;
    fixture.world.plants[0].position.x = -HALF_SIZE.x;

    let tick = fixture.tick();
    assert!(tick.events.is_empty());
    let body = &fixture.world.animals[&id(1)];
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - 5.0, 0.0));
    assert!(body.position.x + body.radius() <= HALF_SIZE.x);
}

#[test]
fn parent_senses_ripe_food_and_newborn_waits_to_process_it() {
    use crate::{
        actor::{Actor, herbivore::Herbivores},
        context::Cpu,
        mind::rnn::RnnGenome,
    };

    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    let mut population = Herbivores::with_immigrant_genome(genome);
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(7),
    };
    let mut world = Ecosystem::empty(1, 7);
    let mut handles = HandleAllocator::new();
    population
        .observe(
            &mut cx,
            world
                .admit(
                    1,
                    Admission {
                        position: Some(DVec2::ZERO),
                        energy: 390.0,
                    },
                    &mut handles,
                )
                .unwrap(),
        )
        .unwrap();
    world.seed_plants(Some(DVec2::ZERO)).unwrap();
    let before = world.clone();
    let observations = world.observe();
    assert_eq!(world, before, "sensing must not change the world");
    let PopulationObservation::Inputs { inputs, .. } = &observations else {
        panic!()
    };
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].1.strength, 100.0);
    let sensed = inputs[0].1;

    population.observe(&mut cx, observations).unwrap();
    let actions = population.act(&mut cx).unwrap();
    let (first, feedback) = world
        .advance(1, actions, &mut handles, |_| Ok(Immigration::default()))
        .unwrap();
    population.observe(&mut cx, feedback).unwrap();
    assert_eq!(first.decisions.len(), 1);
    assert_eq!(first.decisions[0].id, id(1));
    assert_eq!(first.decisions[0].energy, 390.0);
    assert_eq!(first.decisions[0].observation, sensed);
    assert_eq!(world.animals[&id(1)].energy, 213.0);
    assert_eq!(world.animals[&id(2)].energy, 213.0);
    assert_ne!(population.get(id(1)).unwrap().mind.memory, [0.0; 4]);
    assert_eq!(population.get(id(2)).unwrap().mind.memory, [0.0; 4]);

    population.observe(&mut cx, world.observe()).unwrap();
    let actions = population.act(&mut cx).unwrap();
    let (second, feedback) = world
        .advance(2, actions, &mut handles, |_| Ok(Immigration::default()))
        .unwrap();
    population.observe(&mut cx, feedback).unwrap();
    assert_eq!(
        second
            .decisions
            .iter()
            .map(|decision| decision.id)
            .collect::<Vec<_>>(),
        [id(1), id(2)]
    );
    assert_ne!(population.get(id(2)).unwrap().mind.memory, [0.0; 4]);
}

#[test]
fn movement_enters_contact_before_feeding_and_birth() {
    let mut fixture = Fixture::debug(8);
    fixture.action = MoveAction {
        direction: DVec2::X,
        speed_fraction: 1.0,
    };
    fixture.world.plants[0].position = DVec2::new(20.0, 0.0);
    let body = &fixture.world.animals[&id(1)];
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
        (fixture.world.animals[&id(1)].position + fixture.world.animals[&id(2)].position) * 0.5,
        report.decisions[0].position_after
    );
}

#[test]
fn feeding_growth_cannot_leave_a_body_outside_the_arena() {
    let mut fixture = Fixture::debug(9);
    let body = fixture.world.animals.get_mut(&id(1)).unwrap();
    body.energy = START_ENERGY;
    body.position = DVec2::new(HALF_SIZE.x - body.radius(), 0.0);
    fixture.world.plants[0].position = body.position;

    let report = fixture.tick();
    let body = &fixture.world.animals[&id(1)];
    assert_eq!(body.energy, 138.0);
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - body.radius(), 0.0));
    assert!(body.position.x < report.decisions[0].position_after.x);
}

#[test]
fn intervention_sees_post_cull_population_and_immigrants_precede_births() {
    let mut fixture = Fixture::debug(10);
    fixture
        .world
        .spawn(
            &mut fixture.handles,
            Origin::Immigrant,
            DVec2::ZERO,
            -1.0,
            &mut fixture.changes,
        )
        .unwrap();
    let mut queried = None;
    let (report, _) = fixture
        .world
        .advance(
            1,
            vec![(id(1), MoveAction::default())],
            &mut fixture.handles,
            |request| {
                queried = Some(request.population);
                Ok(Immigration { count: 1 })
            },
        )
        .unwrap();
    assert_eq!(queried, Some(1));
    assert!(matches!(report.events[1], Event::Death { id: handle } if handle == id(2)));
    assert!(matches!(report.events[2], Event::Immigrant { id: handle } if handle == id(3)));
    assert!(matches!(report.events[3], Event::Birth { child, .. } if child == id(4)));
    assert_eq!(fixture.world.animals[&id(3)].energy, START_ENERGY - UPKEEP);
}

#[test]
fn malformed_action_batches_fail_before_physical_changes() {
    for actions in [
        vec![],
        vec![
            (id(1), MoveAction::default()),
            (id(1), MoveAction::default()),
        ],
        vec![(id(2), MoveAction::default())],
        vec![(
            id(1),
            MoveAction {
                direction: DVec2::NAN,
                speed_fraction: 0.0,
            },
        )],
    ] {
        let mut fixture = Fixture::debug(11);
        let original = fixture.world.clone();
        assert!(
            fixture
                .world
                .advance(1, actions, &mut fixture.handles, |_| Ok(
                    Immigration::default()
                ))
                .is_err()
        );
        assert_eq!(fixture.world, original);
    }
}

#[test]
fn initial_and_later_arrivals_use_the_same_admission_and_rng_sequence() {
    let mut initial = Ecosystem::empty(0, 42);
    let mut initial_handles = HandleAllocator::new();
    let initial_observation = initial
        .admit(1, Admission::default(), &mut initial_handles)
        .unwrap();

    let mut later = Ecosystem::empty(0, 42);
    let mut later_handles = HandleAllocator::new();
    let (report, later_observation) = later
        .advance(1, vec![], &mut later_handles, |request| {
            assert_eq!(request.population, 0);
            Ok(Immigration { count: 1 })
        })
        .unwrap();
    assert_eq!(initial_observation, later_observation);
    assert_eq!(
        initial.animals[&id(1)].position,
        later.animals[&id(1)].position
    );
    assert_eq!(initial.rng, later.rng);
    assert_eq!(initial_handles, later_handles);
    // Arrivals during a tick also experience that tick's upkeep.
    assert_eq!(
        initial.animals[&id(1)].energy - UPKEEP,
        later.animals[&id(1)].energy
    );
    assert_eq!(report.events, [Event::Immigrant { id: id(1) }]);
    assert!(report.decisions.is_empty());
}
