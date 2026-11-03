#![forbid(unsafe_code)]

use std::{collections::HashSet, env, process::ExitCode, time::Instant};

use nevo::{
    AgentId,
    foraging::{Event, Scenario, TickReport},
    run::{Run, TrialStop, check_prefix, run_trial},
};

const HELP: &str = "\
Usage: forage <trial|continuous|check> [--scenario debug|baseline] [--seed N] [--ticks N]

  trial       Run a fresh trial. With --focal ID, trace the chosen animal's decisions
              and events every tick until death or the tick limit, in either scenario.
              Choose an ID present at the start. Without --focal, the debug scenario
              shows detailed traces for its first three ticks.
  continuous  Keep one world and population alive until --ticks is reached or interrupted.
  check       Compare the trial and continuous prefixes, then advance the continuous run.

Trial and check default to the debug scenario and 32 ticks. Continuous defaults to the
baseline scenario and has no tick limit. The default seed is 1. Counts are cumulative.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Trial,
    Continuous,
    Check,
}

struct Options {
    mode: Mode,
    scenario: Scenario,
    seed: u64,
    ticks: Option<u64>,
    focal: Option<AgentId>,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mode = match args.next().as_deref() {
            Some("trial") => Mode::Trial,
            Some("continuous") => Mode::Continuous,
            Some("check") => Mode::Check,
            Some(name) => return Err(format!("unknown command '{name}'")),
            None => return Err("expected a command".into()),
        };
        let mut options = Self {
            mode,
            scenario: if mode == Mode::Continuous {
                Scenario::Baseline
            } else {
                Scenario::Debug
            },
            seed: 1,
            ticks: None,
            focal: None,
        };
        let mut seen = HashSet::new();
        while let Some(flag) = args.next() {
            if !seen.insert(flag.clone()) {
                return Err(format!("{flag} supplied twice"));
            }
            let next = args.next();
            let value = || {
                next.as_deref()
                    .ok_or_else(|| format!("{flag} needs a value"))
            };
            let number = || {
                let value = value()?;
                value
                    .parse::<u64>()
                    .map_err(|_| format!("{flag} expects an unsigned integer, got '{value}'"))
            };
            match flag.as_str() {
                "--scenario" => options.scenario = value()?.parse()?,
                "--seed" => options.seed = number()?,
                "--ticks" => options.ticks = Some(number()?),
                "--focal" if mode == Mode::Trial => options.focal = Some(AgentId(number()?)),
                "--focal" => return Err("--focal applies only to trials".into()),
                _ => return Err(format!("unknown option '{flag}'")),
            }
        }
        Ok(options)
    }
}

#[derive(Default)]
struct Counts {
    meals: u64,
    births: u64,
    deaths: u64,
    immigrants: u64,
}

impl Counts {
    fn record(&mut self, report: &TickReport) {
        for event in &report.events {
            match event {
                Event::Meal { .. } => self.meals += 1,
                Event::Birth { .. } => self.births += 1,
                Event::Death { .. } => self.deaths += 1,
                Event::Immigrant { .. } => self.immigrants += 1,
            }
        }
    }

    fn print(&self, run: &Run) {
        println!(
            "tick={} animals={} plants={} meals={} births={} deaths={} immigrants={}",
            run.world.number(),
            run.world.animals().len(),
            run.world.plants().len(),
            self.meals,
            self.births,
            self.deaths,
            self.immigrants,
        );
    }
}

fn print_trace(report: &TickReport, focal: Option<AgentId>) {
    for event in &report.events {
        let include = focal.is_none_or(|focal| match *event {
            Event::Meal { id, .. } | Event::Death { id } | Event::Immigrant { id } => id == focal,
            Event::Birth { parent, child } => parent == focal || child == focal,
        });
        if include {
            println!("tick={} event={event:?}", report.tick);
        }
    }
    for decision in &report.decisions {
        if focal.is_none_or(|id| decision.id == id) {
            println!("tick={} decision={decision:?}", report.tick);
        }
    }
}

fn execute(options: Options) -> Result<(), String> {
    let Options {
        mode,
        scenario,
        seed,
        ticks,
        focal,
    } = options;
    let budget = ticks.unwrap_or(32);
    let limit = if mode == Mode::Continuous && ticks.is_none() {
        "unbounded".into()
    } else {
        budget.to_string()
    };
    println!("mode={mode:?} scenario={scenario:?} seed={seed} ticks={limit}");
    if mode == Mode::Check {
        check_prefix(scenario, seed, budget)?;
        println!(
            "Matched the trial and continuous prefixes for {limit} ticks and verified continuation."
        );
        return Ok(());
    }
    let mut run = Run::new(scenario, seed);
    let mut counts = Counts::default();
    if let Some(id) = focal
        && !run.world.animals().contains_key(&id)
    {
        return Err(format!("focal ID {} is absent at trial start", id.0));
    }
    counts.print(&run);
    if mode == Mode::Trial {
        let stop = run_trial(&mut run, budget, |state, report| {
            counts.record(report);
            if focal.is_some() || (scenario == Scenario::Debug && report.tick <= 3) {
                print_trace(report, focal);
            }
            focal.filter(|id| !state.world.animals().contains_key(id))
        });
        match stop {
            TrialStop::TickBudget => println!("trial_stop=tick_budget"),
            TrialStop::Condition(id) => println!("trial_stop=focal_death id={}", id.0),
        }
        counts.print(&run);
    } else {
        let mut last_report = Instant::now();
        while ticks.is_none_or(|limit| run.world.number() < limit) {
            let report = run.tick();
            counts.record(&report);
            let finished = ticks == Some(report.tick);
            if finished || last_report.elapsed().as_secs() >= 2 {
                counts.print(&run);
                last_report = Instant::now();
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let options = match Options::parse(args.into_iter()) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    match execute(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
