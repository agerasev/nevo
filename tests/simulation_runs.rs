#[allow(dead_code)]
mod support;

use std::time::Duration;

use nevo::{
    ecosystem::{Config, Event, Placement},
    run::{Control, LoopOptions, Run, StopReason},
    universe::Universe,
};

#[test]
fn initialization_settings_control_independent_parts_of_the_world() {
    let mut config: Config = Config {
        initial_animals: 2,
        plants: 7,
        minimum_population: 1,
        ..Config::default()
    };
    config.immigration.energy = 0.225;
    config.immigration.position = Placement::Fixed(glam::DVec2::new(10.0, 20.0));
    config.plant.initial_position = Placement::Fixed(glam::DVec2::new(-20.0, 12.0));
    let universe = Universe::new(config.clone(), 7).unwrap();
    let world = universe.world();
    assert_eq!(world.animals().len(), 2);
    assert_eq!(world.plants().len(), 7);
    assert_eq!(world.minimum_population(), 1);
    for animal in world.animals().values() {
        assert_eq!(animal.body.energy, 0.225);
        assert_eq!(animal.body.position, glam::DVec2::new(10.0, 20.0));
    }
    for plant in world.plants() {
        assert_eq!(plant.position, glam::DVec2::new(-20.0, 12.0));
    }
    // Placement validation applies even if the corresponding collection is empty.
    config.initial_animals = 0;
    config.immigration.position = Placement::Fixed(glam::DVec2::splat(f64::NAN));
    assert!(Universe::new(config, 7).is_err());
}

#[test]
fn initial_and_later_immigrants_share_energy_position_and_shape() {
    let mut config: Config = Config {
        initial_animals: 1,
        plants: 0,
        minimum_population: 1,
        ..Config::default()
    };
    config.immigration.energy = 0.0025;
    config.immigration.position = Placement::Fixed(glam::DVec2::new(10.0, 20.0));
    config.animal.body.max_speed = 0.0;
    config.animal.body.upkeep = 0.005;
    config.animal.body.radius_scale = 6.0;
    config.genetics.initialization.hidden_units = 7;
    let mut run = Run::new(config, 31).unwrap();
    {
        let universe = run.universe().read();
        let animal = universe.world().animals().values().next().unwrap();
        assert_eq!(animal.body.energy, 0.0025);
        assert_eq!(animal.body.position, glam::DVec2::new(10.0, 20.0));
        assert!((animal.body.radius() - 0.3).abs() < 1e-12);
        assert_eq!(animal.mind.brain.memory.len(), 7);
    }
    assert_eq!(run.tick().unwrap().decisions.len(), 1);
    for expected_id in 2..=4 {
        let report = run.tick().unwrap();
        assert!(report.decisions.is_empty(), "arrivals first act next tick");
        assert!(report.events.iter().any(|event| {
            matches!(event, Event::Immigrant { id } if id.serial() == expected_id)
        }));
        let universe = run.universe().read();
        let animal = universe.world().animals().values().next().unwrap();
        // Every arrival pays the same tick's upkeep.
        assert_eq!(animal.body.energy, 0.0025 - 0.005);
        assert_eq!(animal.body.position, glam::DVec2::new(10.0, 20.0));
        assert_eq!(animal.mind.brain.genome.hidden_units, 7);
        assert_eq!(animal.mind.brain.memory, vec![0.0; 7]);
    }
}

fn bounded(ticks: u64) -> LoopOptions {
    LoopOptions {
        ticks: Some(ticks),
        interval: Duration::ZERO,
    }
}

#[test]
fn agents_sense_before_feeding_and_newborns_act_next_tick() {
    let mut run = Run::new(support::ecosystem("debug"), 1).unwrap();
    let (parent, genome) = {
        let universe = run.universe().read();
        let parent = *universe.world().animals().keys().next().unwrap();
        (
            parent,
            universe.world().animals()[&parent]
                .mind
                .brain
                .genome
                .clone(),
        )
    };

    let report = run.tick().unwrap();
    let child = report
        .events
        .iter()
        .find_map(|event| match event {
            Event::Birth { child, .. } => Some(*child),
            _ => None,
        })
        .unwrap();
    assert_eq!((parent.serial(), child.serial()), (1, 2));
    assert_eq!(
        report.events,
        [
            Event::Meal {
                id: parent,
                plant: 1,
                gain: 0.1
            },
            Event::Birth { parent, child },
        ]
    );
    assert_eq!(report.decisions.len(), 1);
    assert_eq!(
        (report.decisions[0].id, report.decisions[0].energy),
        (parent, 0.975)
    );
    assert_eq!(report.decisions[0].observation.strength, 100.0);
    {
        let universe = run.universe().read();
        for id in [parent, child] {
            assert_eq!(universe.world().animals()[&id].body.energy, 0.5325);
        }
        assert_ne!(
            universe.world().animals()[&parent].mind.brain.memory,
            [0.0; 4]
        );
        assert_eq!(
            universe.world().animals()[&child].mind.brain.memory,
            [0.0; 4]
        );
        assert_eq!(
            universe.world().animals()[&parent].mind.brain.genome,
            genome
        );
        assert_ne!(universe.world().animals()[&child].mind.brain.genome, genome);
    }
    assert!(
        run.tick()
            .unwrap()
            .decisions
            .iter()
            .any(|decision| decision.id == child)
    );
    assert_ne!(
        run.universe().read().world().animals()[&child]
            .mind
            .brain
            .memory,
        [0.0; 4]
    );
}

#[test]
fn limits_and_stop_conditions_preserve_every_tick_and_continuation() {
    for config in [
        support::ecosystem::<Config>("debug"),
        support::ecosystem::<Config>("baseline"),
    ] {
        let mut limited = Run::new(config.clone(), 42).unwrap();
        let mut reports = Vec::new();
        let (_control, commands) = Control::channel(false);
        assert_eq!(
            limited
                .execute(bounded(128), commands, |_, report| {
                    reports.push(report.clone());
                    false
                })
                .unwrap(),
            StopReason::TickLimit,
        );

        let mut conditional = Run::new(config.clone(), 42).unwrap();
        let (_control, commands) = Control::channel(false);
        assert_eq!(
            conditional
                .execute(
                    LoopOptions {
                        ticks: None,
                        ..bounded(128)
                    },
                    commands,
                    |state, report| {
                        let tick = state.universe().read().step();
                        assert_eq!(report, &reports[(tick - 1) as usize]);
                        tick == 128
                    },
                )
                .unwrap(),
            StopReason::Condition,
        );
        assert_eq!(limited, conditional);
        assert_eq!(limited.tick().unwrap(), conditional.tick().unwrap());
        assert_eq!(limited, conditional);
    }
}

#[test]
fn a_condition_stop_can_resume_the_same_run() {
    let mut resumed = Run::new(support::ecosystem("debug"), 1).unwrap();
    let mut uninterrupted = resumed.clone();
    let (_control, commands) = Control::channel(false);
    let stop = resumed
        .execute(bounded(32), commands, |_, report| {
            report
                .events
                .iter()
                .any(|event| matches!(event, Event::Birth { .. }))
        })
        .unwrap();
    assert_eq!(stop, StopReason::Condition);
    assert_eq!(resumed.universe().read().step(), 1);
    let (_control, commands) = Control::channel(false);
    assert_eq!(
        resumed
            .execute(bounded(31), commands, |_, _| false)
            .unwrap(),
        StopReason::TickLimit
    );
    let (_control, commands) = Control::channel(false);
    assert_eq!(
        uninterrupted
            .execute(bounded(32), commands, |_, _| false)
            .unwrap(),
        StopReason::TickLimit
    );
    assert_eq!(resumed, uninterrupted);
}

#[test]
fn minimum_population_is_an_experiment_setting() {
    let mut unsupported = Universe::new(support::ecosystem("debug"), 1).unwrap();
    let mut supported_world = unsupported.world().clone();
    supported_world.set_minimum_population(20);
    assert_eq!(unsupported.world().animals(), supported_world.animals());
    assert_eq!(unsupported.world().plants(), supported_world.plants());
    let mut supported = Universe::from_world(supported_world).unwrap();

    let unsupported_report = unsupported.advance().unwrap();
    let supported_report = supported.advance().unwrap();
    assert!(
        !unsupported_report
            .events
            .iter()
            .any(|event| matches!(event, Event::Immigrant { .. }))
    );
    assert_eq!(
        supported_report
            .events
            .iter()
            .filter(|event| matches!(event, Event::Immigrant { .. }))
            .count(),
        19
    );
    assert_eq!(unsupported.world().animals().len(), 2);
    assert_eq!(supported.world().animals().len(), 21);
    for animal in supported.world().animals().values() {
        assert!(animal.body.energy.is_finite());
        assert!(
            animal
                .mind
                .brain
                .genome
                .weights
                .iter()
                .all(|weight| weight.is_finite())
        );
    }
}
