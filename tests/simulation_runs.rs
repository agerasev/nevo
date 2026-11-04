use std::time::Duration;

use nevo::{
    ecosystem::{Event, Scenario},
    run::{Control, LoopOptions, Run, StopReason},
    universe::Universe,
};

fn bounded(ticks: u64) -> LoopOptions {
    LoopOptions {
        ticks: Some(ticks),
        interval: Duration::ZERO,
    }
}

#[test]
fn agents_sense_before_feeding_and_newborns_act_next_tick() {
    let mut run = Run::new(Scenario::Debug, 1).unwrap();
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
                gain: 40.0
            },
            Event::Birth { parent, child },
        ]
    );
    assert_eq!(report.decisions.len(), 1);
    assert_eq!(
        (report.decisions[0].id, report.decisions[0].energy),
        (parent, 390.0)
    );
    assert_eq!(report.decisions[0].observation.strength, 100.0);
    {
        let universe = run.universe().read();
        for id in [parent, child] {
            assert_eq!(universe.world().animals()[&id].body.energy, 213.0);
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
    for scenario in [Scenario::Debug, Scenario::Baseline] {
        let mut limited = Run::new(scenario, 42).unwrap();
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

        let mut conditional = Run::new(scenario, 42).unwrap();
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
    let mut resumed = Run::new(Scenario::Debug, 1).unwrap();
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
    let mut unsupported = Universe::new(Scenario::Debug, 1).unwrap();
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
