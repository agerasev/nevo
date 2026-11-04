use std::time::Duration;

use nevo::{
    AgentId,
    foraging::{Event, Scenario},
    run::{Control, LoopOptions, Run, StopReason},
};

fn bounded(ticks: u64) -> LoopOptions {
    LoopOptions {
        ticks: Some(ticks),
        interval: Duration::ZERO,
    }
}

#[test]
fn agents_sense_before_feeding_and_newborns_act_next_tick() {
    let mut run = Run::new(Scenario::Debug, 1);
    let (parent, child) = (AgentId(1), AgentId(2));
    let genome = run.population.get(parent).unwrap().genome.clone();

    let report = run.tick();
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
    for id in [parent, child] {
        assert_eq!(run.world().read().animals()[&id].energy, 213.0);
    }
    assert_ne!(run.population.get(parent).unwrap().memory, [0.0; 4]);
    assert_eq!(run.population.get(child).unwrap().memory, [0.0; 4]);
    assert_eq!(run.population.get(parent).unwrap().genome, genome);
    assert_ne!(run.population.get(child).unwrap().genome, genome);
    assert!(
        run.tick()
            .decisions
            .iter()
            .any(|decision| decision.id == child)
    );
    assert_ne!(run.population.get(child).unwrap().memory, [0.0; 4]);
}

#[test]
fn limits_and_stop_conditions_preserve_every_tick_and_continuation() {
    for scenario in [Scenario::Debug, Scenario::Baseline] {
        let mut limited = Run::new(scenario, 42);
        let mut reports = Vec::new();
        let (_control, commands) = Control::channel(false);
        assert_eq!(
            limited.execute(bounded(128), commands, |_, report| {
                reports.push(report.clone());
                false
            }),
            StopReason::TickLimit,
        );

        let mut conditional = Run::new(scenario, 42);
        let (_control, commands) = Control::channel(false);
        assert_eq!(
            conditional.execute(
                LoopOptions {
                    ticks: None,
                    ..bounded(128)
                },
                commands,
                |state, report| {
                    let tick = state.world().read().number();
                    assert_eq!(report, &reports[(tick - 1) as usize]);
                    tick == 128
                },
            ),
            StopReason::Condition,
        );
        assert_eq!(limited, conditional);
        assert_eq!(limited.tick(), conditional.tick());
        assert_eq!(limited, conditional);
    }
}

#[test]
fn a_condition_stop_can_resume_the_same_run() {
    let mut resumed = Run::new(Scenario::Debug, 1);
    let mut uninterrupted = resumed.clone();
    let (_control, commands) = Control::channel(false);
    let stop = resumed.execute(bounded(32), commands, |_, report| {
        report
            .events
            .iter()
            .any(|event| matches!(event, Event::Birth { .. }))
    });
    assert_eq!(stop, StopReason::Condition);
    assert_eq!(resumed.world().read().number(), 1);
    let (_control, commands) = Control::channel(false);
    assert_eq!(
        resumed.execute(bounded(31), commands, |_, _| false),
        StopReason::TickLimit
    );
    let (_control, commands) = Control::channel(false);
    assert_eq!(
        uninterrupted.execute(bounded(32), commands, |_, _| false),
        StopReason::TickLimit
    );
    assert_eq!(resumed, uninterrupted);
}
