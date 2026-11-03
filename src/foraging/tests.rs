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

#[test]
fn first_claimant_gets_the_only_meal_and_replacement_starts_small() {
    let mut population = Walker::default();
    let mut world = ForagingWorld::new(Scenario::Debug, 1, &mut population);
    let first = AgentId(1);
    world.animals.get_mut(&first).unwrap().energy = START_ENERGY;
    let second = world.spawn(&mut population, Origin::Founder, DVec2::ZERO, START_ENERGY);

    let tick = world.tick(&mut population);
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
fn upkeep_defers_death_until_the_next_feeding_phase_and_replacement_acts() {
    let mut population = Walker::default();
    let mut world = ForagingWorld::new(Scenario::Debug, 2, &mut population);
    world.immigrant_floor = 1;
    world.plants[0].position = DVec2::new(100.0, 0.0);
    world.animals.get_mut(&AgentId(1)).unwrap().energy = 1.0;

    let starving = world.tick(&mut population);
    assert!(starving.events.is_empty());
    assert_eq!(starving.decisions[0].energy, -1.0);
    assert!(starving.decisions[0].position_after.is_finite());

    world.plants[0].position = starving.decisions[0].position_after;
    let replacement = world.tick(&mut population);
    assert_eq!(
        replacement.events,
        [
            Event::Death { id: AgentId(1) },
            Event::Immigrant { id: AgentId(2) }
        ]
    );
    assert_eq!(population.departures, [AgentId(1)]);
    assert_eq!(population.arrivals[1], (AgentId(2), Origin::Immigrant));
    assert_eq!(replacement.decisions.len(), 1);
    assert_eq!(
        (replacement.decisions[0].id, replacement.decisions[0].energy),
        (AgentId(2), 98.0)
    );
}

#[test]
fn field_uses_maturity_distance_and_the_world_axes() {
    let mut population = Walker::default();
    let mut world = ForagingWorld::new(Scenario::Debug, 3, &mut population);
    world.plants[0].position = DVec2::new(70.0, 0.0);
    world.add_plant(DVec2::new(-70.0, 0.0), RIPE_ENERGY);
    let balanced = world.observe(DVec2::ZERO);
    assert_eq!(balanced.gradient, DVec2::ZERO);
    assert!((balanced.strength - 1.0 / 0.11).abs() < 1e-12);

    world.plants[1].energy *= 0.5;
    let unbalanced = world.observe(DVec2::ZERO);
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
    let mut world = ForagingWorld::new(Scenario::Debug, 4, &mut population);
    let body = world.animals.get_mut(&AgentId(1)).unwrap();
    body.position.x = HALF_SIZE.x - 5.0;
    body.energy = START_ENERGY;
    world.plants[0].position.x = -HALF_SIZE.x;

    let tick = world.tick(&mut population);
    assert!(tick.events.is_empty());
    let body = &world.animals[&AgentId(1)];
    assert_eq!(body.position, DVec2::new(HALF_SIZE.x - body.radius(), 0.0));
}
