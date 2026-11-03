use rand::{Rng, SeedableRng};
use rand_xoshiro::SplitMix64;

use crate::{
    Environment,
    foraging::{ForagingWorld, Scenario, TickReport},
    herbivore::Herbivores,
};

/// Keep this pair alive to continue the same trajectory across trial boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct Run<W = ForagingWorld, P = Herbivores> {
    pub world: W,
    pub population: P,
}

impl Run {
    pub fn new(scenario: Scenario, seed: u64) -> Self {
        let mut seeds = SplitMix64::seed_from_u64(seed);
        let world_seed = seeds.next_u64();
        let mut population = Herbivores::new(seeds.next_u64());
        let world = ForagingWorld::new(scenario, world_seed, &mut population);
        Self { world, population }
    }
}

impl<W: Environment<P>, P> Run<W, P> {
    pub fn tick(&mut self) -> W::Report {
        self.world.tick(&mut self.population)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TrialStop<R> {
    TickBudget,
    Condition(R),
}

/// Advance an existing run. The caller chooses which observation ends the trial.
pub fn run_trial<W: Environment<P>, P, R>(
    run: &mut Run<W, P>,
    budget: u64,
    mut after_tick: impl FnMut(&Run<W, P>, &W::Report) -> Option<R>,
) -> TrialStop<R> {
    for _ in 0..budget {
        let report = run.tick();
        if let Some(reason) = after_tick(run, &report) {
            return TrialStop::Condition(reason);
        }
    }
    TrialStop::TickBudget
}

/// Compare every trial tick against an independently initialized continuous run.
pub fn check_prefix(scenario: Scenario, seed: u64, ticks: u64) -> Result<(), String> {
    let mut trial = Run::new(scenario, seed);
    let mut continuous = Run::new(scenario, seed);
    if trial != continuous {
        return Err("initial states differ".into());
    }
    let stop = run_trial(&mut trial, ticks, |trial, report| {
        let other = continuous.tick();
        compare_tick(trial, report, &continuous, &other).err()
    });
    if let TrialStop::Condition(error) = stop {
        return Err(error);
    }
    if trial.world.number() != ticks {
        return Err(format!(
            "trial stopped at tick {}, expected {ticks}",
            trial.world.number()
        ));
    }
    // Removing the budget must also preserve the next decision of a resumed trial.
    let next = continuous.tick();
    let resumed = trial.tick();
    compare_tick(&trial, &resumed, &continuous, &next)
}

fn compare_tick(
    trial: &Run,
    report: &TickReport,
    continuous: &Run,
    other: &TickReport,
) -> Result<(), String> {
    let difference = if report != other {
        "events or decisions"
    } else if trial.world != continuous.world {
        "world state (including spatial RNG)"
    } else if trial.population != continuous.population {
        "genomes, controller memories or genetic RNG"
    } else {
        return Ok(());
    };
    Err(format!("tick {} differs in {difference}", report.tick))
}
