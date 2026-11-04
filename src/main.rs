#![forbid(unsafe_code)]

mod config;
mod persistence;

#[cfg(feature = "viewer")]
mod smoke;
#[cfg(feature = "viewer")]
mod viewer;

use std::{
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, ensure};
use nevo::{
    foraging::{Event, Scenario, TickReport},
    run::{Command, Control, Run, StopReason},
};
use serde::{Deserialize, Serialize};

use config::Options;
use persistence::RunDirectory;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

fn print_trace(report: &TickReport) {
    for event in &report.events {
        println!("tick={} event={event:?}", report.tick);
    }
    for decision in &report.decisions {
        println!("tick={} decision={decision:?}", report.tick);
    }
}

struct Worker {
    control: Control,
    thread: Option<thread::JoinHandle<Result<StopReason>>>,
}

impl Worker {
    fn join(mut self) -> Result<StopReason> {
        self.thread
            .take()
            .expect("worker can be joined once")
            .join()
            .map_err(|_| anyhow!("simulation worker panicked"))?
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
    smoke: Option<smoke::SmokeCheck>,
    exit_requested: Arc<AtomicBool>,
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
                    smoke.clone(),
                    exit_requested.clone(),
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

fn execute(mut options: Options) -> Result<()> {
    options.loop_options()?;
    let directory = RunDirectory::open(options.directory.as_deref())?;
    // Reload the saved configuration under the writer lock in case another
    // invocation finished between argument parsing and acquiring the directory.
    if options.directory.is_some() {
        options = Options::load()?;
    }
    let (mut run, mut counts) = match directory.load_latest()? {
        Some(checkpoint) => {
            options = Options::load_with_fallback(&checkpoint.config)?;
            ensure!(
                options.scenario == checkpoint.config.scenario
                    && options.seed == checkpoint.config.seed,
                "cannot change the seed or scenario of an existing run"
            );
            let run = checkpoint.state.restore().context("restoring checkpoint")?;
            eprintln!(
                "resuming={} tick={}",
                directory.path().display(),
                run.world().read().number()
            );
            (run, checkpoint.counts)
        }
        None => {
            directory.check_new()?;
            options.seed = Some(options.resolved_seed()?);
            (
                Run::new(options.scenario, options.seed.unwrap()),
                Counts::default(),
            )
        }
    };
    let limits = options.loop_options()?;
    let seed = options
        .seed
        .expect("new and restored runs have a resolved seed");
    // No configuration is overwritten until the checkpoint is decoded and validated.
    directory.save_config(&options)?;
    eprintln!("run={}", directory.path().display());
    println!(
        "scenario={:?} seed={} ticks={:?} interval={:?}",
        options.scenario, seed, limits.ticks, limits.interval,
    );
    let (control, commands) = Control::channel(options.paused || options.smoke);
    let exit_requested = Arc::new(AtomicBool::new(false));
    let signal_exit = Arc::clone(&exit_requested);
    let signal_control = control.clone();
    ctrlc::set_handler(move || {
        signal_exit.store(true, Ordering::Relaxed);
        let _ = signal_control.send(Command::Stop);
    })
    .context("installing shutdown handler")?;
    #[cfg(feature = "viewer")]
    let world = run.world().clone();
    #[cfg(feature = "viewer")]
    let smoke = options.smoke.then(|| smoke::SmokeCheck::new(run.clone()));
    let worker_options = options.clone();
    let worker_exit = Arc::clone(&exit_requested);
    let thread = thread::spawn(move || {
        let options = worker_options;
        counts.print(&run);
        let mut last_report = Instant::now();
        let mut save_error = None;
        let stop = run.execute(limits, commands, |state, report| {
            counts.record(report);
            if options.scenario == Scenario::Debug && report.tick <= 3 {
                print_trace(report);
            }
            if last_report.elapsed() >= Duration::from_secs(2) {
                counts.print(state);
                last_report = Instant::now();
            }
            if options.checkpoint_every != 0
                && report.tick.is_multiple_of(options.checkpoint_every)
                && let Err(error) = directory.save(state, &options, &counts)
            {
                save_error = Some(error);
                return true;
            }
            false
        });
        counts.print(&run);
        println!("stop={stop:?}");
        let saved = match save_error {
            Some(error) => Err(error),
            None => directory.save(&run, &options, &counts),
        };
        if saved.is_err() {
            worker_exit.store(true, Ordering::Relaxed);
        }
        saved?;
        Ok(stop)
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
            seed,
            smoke,
            exit_requested,
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
    viewed.map_err(anyhow::Error::msg)?;
    Ok(())
}

fn main() -> ExitCode {
    let options = match Options::load() {
        Ok(options) => options,
        Err(error) => {
            if let Some(parse_error) = error.downcast_ref::<conf::Error>() {
                parse_error.exit();
            }
            eprintln!("{error:#}");
            return ExitCode::from(2);
        }
    };
    match execute(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
