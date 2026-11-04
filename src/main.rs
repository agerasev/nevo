#![forbid(unsafe_code)]

#[cfg(feature = "viewer")]
mod smoke;
#[cfg(feature = "viewer")]
mod viewer;

use std::{
    collections::HashSet,
    env,
    process::ExitCode,
    thread,
    time::{Duration, Instant},
};

use nevo::{
    AgentId,
    foraging::{Event, Scenario, TickReport},
    run::{Control, LoopOptions, Run, StopReason},
};

#[cfg(feature = "viewer")]
use nevo::run::Command;

const HELP: &str = "\
Usage: nevo [--viewer] [--scenario baseline|debug] [--seed N] [--ticks N]
            [--seconds N] [--hz N] [--paused] [--focal ID] [--smoke]

Run one seeded experiment, optionally attaching a viewer. Defaults: baseline,
seed 1, 25 ticks/second, no time limit. --hz 0 runs as fast as possible.
--ticks limits simulation steps; --seconds limits elapsed time, including pauses.
--focal traces an initially present animal and stops when it dies.

The viewer reads world snapshots. Space pauses/resumes and Right Arrow advances
one tick when the arena has focus. --paused starts at tick zero.
--viewer --smoke checks twelve displayed snapshots against an independent run.";

struct Options {
    scenario: Scenario,
    seed: u64,
    viewer: bool,
    paused: bool,
    smoke: bool,
    focal: Option<AgentId>,
    limits: LoopOptions,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self {
            scenario: Scenario::Baseline,
            seed: 1,
            viewer: false,
            paused: false,
            smoke: false,
            focal: None,
            limits: LoopOptions::default(),
        };
        let mut seen = HashSet::new();
        while let Some(flag) = args.next() {
            if !seen.insert(flag.clone()) {
                return Err(format!("{flag} supplied twice"));
            }
            match flag.as_str() {
                "--viewer" => options.viewer = true,
                "--paused" => options.paused = true,
                "--smoke" => options.smoke = true,
                "--scenario" => {
                    options.scenario = args.next().ok_or("--scenario needs a name")?.parse()?;
                }
                "--seed" | "--ticks" | "--focal" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} needs a number"))?;
                    let number = value.parse::<u64>().map_err(|_| {
                        format!("{flag} expects an unsigned integer, got '{value}'")
                    })?;
                    match flag.as_str() {
                        "--seed" => options.seed = number,
                        "--ticks" => options.limits.ticks = Some(number),
                        "--focal" => options.focal = Some(AgentId(number)),
                        _ => unreachable!(),
                    }
                }
                "--seconds" | "--hz" => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{flag} needs a number"))?;
                    let number = value
                        .parse::<f64>()
                        .map_err(|_| format!("invalid {flag} value '{value}'"))?;
                    if !number.is_finite() || number < 0.0 {
                        return Err(format!("{flag} must be finite and nonnegative"));
                    }
                    if flag == "--seconds" {
                        options.limits.duration = Some(
                            Duration::try_from_secs_f64(number)
                                .map_err(|_| "--seconds is too large")?,
                        );
                    } else {
                        options.limits.interval = if number == 0.0 {
                            Duration::ZERO
                        } else {
                            let interval = Duration::try_from_secs_f64(1.0 / number)
                                .map_err(|_| "--hz is too small")?;
                            if interval.is_zero() {
                                return Err("--hz is too large".into());
                            }
                            interval
                        };
                    }
                }
                _ => return Err(format!("unknown option '{flag}'")),
            }
        }
        if options.viewer && !cfg!(feature = "viewer") {
            return Err("this build does not include the viewer feature".into());
        }
        if options.smoke && !options.viewer {
            return Err("--smoke requires --viewer".into());
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
        let world = run.world().read();
        println!(
            "tick={} animals={} plants={} meals={} births={} deaths={} immigrants={}",
            world.number(),
            world.animals().len(),
            world.plants().len(),
            self.meals,
            self.births,
            self.deaths,
            self.immigrants,
        );
    }
}

fn print_trace(report: &TickReport, focal: Option<AgentId>) {
    for event in &report.events {
        if focal.is_none_or(|focal| match *event {
            Event::Meal { id, .. } | Event::Death { id } | Event::Immigrant { id } => id == focal,
            Event::Birth { parent, child } => parent == focal || child == focal,
        }) {
            println!("tick={} event={event:?}", report.tick);
        }
    }
    for decision in &report.decisions {
        if focal.is_none_or(|id| decision.id == id) {
            println!("tick={} decision={decision:?}", report.tick);
        }
    }
}

struct Worker {
    control: Control,
    thread: Option<thread::JoinHandle<StopReason>>,
}

impl Worker {
    fn join(mut self) -> Result<StopReason, String> {
        self.thread
            .take()
            .expect("worker can be joined once")
            .join()
            .map_err(|_| "simulation worker panicked".to_string())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // Window initialization can unwind before the usual shutdown path.
            let _ = self.control.send(nevo::run::Command::Stop);
            let _ = thread.join();
        }
    }
}

#[cfg(feature = "viewer")]
fn attach_viewer(
    world: nevo::run::WorldView<nevo::foraging::ForagingWorld>,
    control: Control,
    scenario: Scenario,
    seed: u64,
    smoke: bool,
) -> Result<(), String> {
    use std::sync::{Arc, Mutex};

    let result = Arc::new(Mutex::new(None));
    let result_out = result.clone();
    wgame::app::entry(async move || {
        let config = wgame::WindowConfig::default()
            .title("NEvo · Foraging")
            .logical_size((1200.0, 900.0))
            .resizable(true)
            .vsync(true);
        let viewed = wgame::app_with_single_window(
            async move |window| {
                viewer::show(
                    window,
                    world.clone(),
                    control.clone(),
                    scenario,
                    seed,
                    smoke,
                )
                .await
            },
            config,
        )
        .await;
        *result_out.lock().unwrap() = Some(viewed.map_err(|error| error.to_string()));
    });
    result
        .lock()
        .unwrap()
        .take()
        .unwrap_or_else(|| Err("viewer exited without a result".into()))
}

fn execute(options: Options) -> Result<(), String> {
    let mut run = Run::new(options.scenario, options.seed);
    if let Some(id) = options.focal
        && !run.world().read().animals().contains_key(&id)
    {
        return Err(format!("focal ID {} is absent at experiment start", id.0));
    }
    println!(
        "scenario={:?} seed={} ticks={:?} duration={:?} interval={:?}",
        options.scenario,
        options.seed,
        options.limits.ticks,
        options.limits.duration,
        options.limits.interval,
    );
    let (control, commands) = Control::channel(options.paused || options.smoke);
    #[cfg(feature = "viewer")]
    let world = run.world().clone();
    let thread = thread::spawn(move || {
        let mut counts = Counts::default();
        counts.print(&run);
        let mut last_report = Instant::now();
        let stop = run.execute(options.limits, commands, |state, report| {
            counts.record(report);
            if options.focal.is_some() || (options.scenario == Scenario::Debug && report.tick <= 3)
            {
                print_trace(report, options.focal);
            }
            if last_report.elapsed() >= Duration::from_secs(2) {
                counts.print(state);
                last_report = Instant::now();
            }
            options
                .focal
                .is_some_and(|id| !state.world().read().animals().contains_key(&id))
        });
        counts.print(&run);
        println!("stop={stop:?}");
        stop
    });
    let worker = Worker {
        control: control.clone(),
        thread: Some(thread),
    };
    #[cfg(feature = "viewer")]
    let viewed = if options.viewer {
        let result = attach_viewer(
            world,
            control.clone(),
            options.scenario,
            options.seed,
            options.smoke,
        );
        let _ = control.send(Command::Stop);
        result
    } else {
        Ok(())
    };
    // Keep the command sender alive while a run without a viewer completes.
    let joined = worker.join();
    drop(control);
    joined?;
    #[cfg(feature = "viewer")]
    viewed?;
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
