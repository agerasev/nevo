use super::*;

#[derive(Default)]
struct Walker {
    action: MoveAction,
    arrivals: Vec<(AgentId, Origin)>,
    departures: Vec<AgentId>,
}

impl Population<FoodObservation, MoveAction, Origin> for Walker {
    fn spawn(&mut self, id: AgentId, origin: Origin) {
        self.arrivals.push((id, origin));
    }

    fn act(&mut self, _: AgentId, _: &FoodObservation) -> MoveAction {
        self.action
    }

    fn despawn(&mut self, id: AgentId) {
        self.departures.push(id);
    }
}

fn tick<P: Population<FoodObservation, MoveAction, Origin>>(
    world: &mut Ecosystem,
    population: &mut P,
) -> TickReport {
    let responses = <Ecosystem as Environment<P>>::observe(world)
        .into_iter()
        .map(|(id, observation)| AgentResponse {
            id,
            action: population.act(id, &observation),
            observation,
        })
        .collect();
    world.update(population, responses)
}

#[test]
fn first_claimant_gets_the_only_meal_and_replacement_starts_small() {
    let mut population = Walker::default();
    let mut world = Ecosystem::new(Scenario::Debug, 1, &mut population);
    let first = AgentId(1);
    world.animals.get_mut(&first).unwrap().energy = START_ENERGY;
    let second = world.spawn(&mut population, Origin::Founder, DVec2::ZERO, START_ENERGY);

    let tick = tick(&mut world, &mut population);
    assert_eq!(
        tick.events,
        [Event::Meal {
            id: first,
            plant: 1,
            gain: 40.0
        }]
    );
    assert_eq!(world.animals[&first].energy, 138.0);
    assert_eq!(world.animals[&second].energy, 98.0);
    assert_eq!(world.plants[0].energy, 1.0);
    assert_ne!(world.plants[0].id, 1);
}

#[test]
fn upkeep_defers_death_and_replacement_first_acts_on_the_following_tick() {
    let mut population = Walker::default();
    let mut world = Ecosystem::new(Scenario::Debug, 2, &mut population);
    world.immigrant_floor = 1;
    world.plants[0].position = DVec2::new(100.0, 0.0);
    world.animals.get_mut(&AgentId(1)).unwrap().energy = 1.0;

    let starving = tick(&mut world, &mut population);
    assert!(starving.events.is_empty());
    assert_eq!(starving.decisions[0].energy, 1.0);
    assert_eq!(world.animals[&AgentId(1)].energy, -1.0);
    assert!(starving.decisions[0].position_after.is_finite());

    world.plants[0].position = starving.decisions[0].position_after;
    let replacement = tick(&mut world, &mut population);
    assert_eq!(
        replacement.events,
        [
            Event::Death { id: AgentId(1) },
            Event::Immigrant { id: AgentId(2) }
        ]
    );
    assert_eq!(population.departures, [AgentId(1)]);
    assert_eq!(population.arrivals[1], (AgentId(2), Origin::Immigrant));
    assert!(replacement.decisions.is_empty());
    let next = tick(&mut world, &mut population);
    assert_eq!(next.decisions.len(), 1);
    assert_eq!(
        (next.decisions[0].id, next.decisions[0].energy),
        (AgentId(2), 98.0)
    );
}

#[test]
fn field_uses_maturity_distance_and_the_world_axes() {
    let mut population = Walker::default();
    let mut world = Ecosystem::new(Scenario::Debug, 3, &mut population);
    world.plants[0].position = DVec2::new(70.0, 0.0);
    world.add_plant(DVec2::new(-70.0, 0.0), RIPE_ENERGY);
    let balanced = world.sense(DVec2::ZERO);
    assert_eq!(balanced.gradient, DVec2::ZERO);
    assert!((balanced.strength - 1.0 / 0.11).abs() < 1e-12);

    world.plants[1].energy *= 0.5;
    let unbalanced = world.sense(DVec2::ZERO);
    assert!((unbalanced.gradient - DVec2::X).length() < 1e-12);
    assert!((unbalanced.strength - 0.75 / 0.11).abs() < 1e-12);
}

#[test]
fn body_stops_at_the_edge_and_cannot_eat_across_it() {
    let mut population = Walker {
        action: MoveAction {
            direction: DVec2::X,
            speed_fraction: 1.0,
        },
        ..Default::default()
    };
    let mut world = Ecosystem::new(Scenario::Debug, 4, &mut population);
    let body = world.animals.get_mut(&AgentId(1)).unwrap();
    body.position.x = HALF_SIZE.x - 5.0;
    body.energy = START_ENERGY;
    world.plants[0].position.x = -HALF_SIZE.x;

    let tick = tick(&mut world, &mut population);
    assert!(tick.events.is_empty());
    let body = &world.animals[&AgentId(1)];
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - 5.0, 0.0));
    assert!(body.position.x + body.radius() <= HALF_SIZE.x);
}

#[test]
fn parent_senses_ripe_food_and_newborn_waits_to_process_it() {
    use crate::{agent::herbivore::Herbivores, mind::rnn::RnnGenome};

    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    let mut population = Herbivores::with_founder_genome(7, genome);
    let mut world = Ecosystem::new(Scenario::Debug, 7, &mut population);
    let before = world.clone();
    let observations = <Ecosystem as Environment<Herbivores>>::observe(&world);
    assert_eq!(world, before, "sensing must not change the world");
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].1.strength, 100.0);

    let first = tick(&mut world, &mut population);
    assert_eq!(first.decisions.len(), 1);
    assert_eq!(first.decisions[0].id, AgentId(1));
    assert_eq!(first.decisions[0].energy, 390.0);
    assert_eq!(first.decisions[0].observation, observations[0].1);
    assert_eq!(world.animals[&AgentId(1)].energy, 213.0);
    assert_eq!(world.animals[&AgentId(2)].energy, 213.0);
    assert_ne!(population.get(AgentId(1)).unwrap().mind.memory, [0.0; 4]);
    assert_eq!(population.get(AgentId(2)).unwrap().mind.memory, [0.0; 4]);

    let second = tick(&mut world, &mut population);
    assert_eq!(
        second
            .decisions
            .iter()
            .map(|decision| decision.id)
            .collect::<Vec<_>>(),
        [AgentId(1), AgentId(2)]
    );
    assert_ne!(population.get(AgentId(2)).unwrap().mind.memory, [0.0; 4]);
}

#[test]
fn movement_enters_contact_before_feeding_and_birth() {
    let mut population = Walker {
        action: MoveAction {
            direction: DVec2::X,
            speed_fraction: 1.0,
        },
        ..Default::default()
    };
    let mut world = Ecosystem::new(Scenario::Debug, 8, &mut population);
    world.plants[0].position = DVec2::new(20.0, 0.0);
    let body = &world.animals[&AgentId(1)];
    assert!(
        body.position.distance(world.plants[0].position)
            >= 0.8 * (body.radius() + world.plants[0].radius())
    );

    let report = tick(&mut world, &mut population);
    assert_eq!(report.decisions.len(), 1);
    assert_eq!(report.decisions[0].position_before, DVec2::ZERO);
    assert_eq!(report.decisions[0].position_after, DVec2::new(10.0, 0.0));
    assert_eq!(
        report.events,
        [
            Event::Meal {
                id: AgentId(1),
                plant: 1,
                gain: 40.0,
            },
            Event::Birth {
                parent: AgentId(1),
                child: AgentId(2),
            },
        ]
    );
    assert_eq!(
        (world.animals[&AgentId(1)].position + world.animals[&AgentId(2)].position) * 0.5,
        report.decisions[0].position_after
    );
}

#[test]
fn feeding_growth_cannot_leave_a_body_outside_the_arena() {
    let mut population = Walker::default();
    let mut world = Ecosystem::new(Scenario::Debug, 9, &mut population);
    let body = world.animals.get_mut(&AgentId(1)).unwrap();
    body.energy = START_ENERGY;
    body.position = DVec2::new(HALF_SIZE.x - body.radius(), 0.0);
    world.plants[0].position = body.position;

    let report = tick(&mut world, &mut population);
    let body = &world.animals[&AgentId(1)];
    assert_eq!(body.energy, 138.0);
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - body.radius(), 0.0));
    assert!(body.position.x < report.decisions[0].position_after.x);
}
