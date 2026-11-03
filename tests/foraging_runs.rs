use nevo::{
    AgentId,
    foraging::{Event, Scenario},
    run::{Run, TrialStop, check_prefix, run_trial},
};

#[test]
fn meal_and_split_happen_before_both_animals_act() {
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
            Event::Birth { parent, child }
        ]
    );
    assert_eq!(report.decisions.len(), 2);
    for (decision, id) in report.decisions.iter().zip([parent, child]) {
        assert_eq!((decision.id, decision.energy), (id, 213.0));
        assert_eq!(run.world.animals()[&id].energy, 213.0);
        assert_ne!(run.population.get(id).unwrap().memory, [0.0; 4]);
    }
    assert_eq!(run.population.get(parent).unwrap().genome, genome);
    assert_ne!(run.population.get(child).unwrap().genome, genome);
}

#[test]
fn trial_matches_continuous_through_births_and_deaths() {
    check_prefix(Scenario::Debug, 1, 128).unwrap();
    check_prefix(Scenario::Baseline, 1, 128).unwrap();
}

#[test]
fn a_trial_boundary_preserves_the_live_run() {
    let mut trial = Run::new(Scenario::Debug, 1);
    let mut continuous = Run::new(Scenario::Debug, 1);
    let stop = run_trial(&mut trial, 32, |_, report| {
        report.events.iter().find_map(|event| match event {
            Event::Birth { child, .. } => Some(*child),
            _ => None,
        })
    });
    assert_eq!(stop, TrialStop::Condition(AgentId(2)));
    assert_eq!(trial.world.number(), 1);
    continuous.tick();
    assert_eq!(trial.tick(), continuous.tick());
    assert_eq!(trial, continuous);
}
