mod cli;
mod config;
mod metrics;
mod persistence;

#[cfg(feature = "viewer")]
mod smoke;

#[cfg(feature = "viewer")]
use nevo::viewer::{self, Draw, Label};

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
use clap::Parser;
use nevo::{
    ecosystem::TickReport,
    run::{Command, Control, Run, StopReason},
};

use cli::Cli;
use metrics::MetricsLog;
use persistence::RunDirectory;

fn print_progress(run: &Run) {
    let universe = run.universe().read();
    let world = universe.world();
    println!(
        "tick={} animals={} plants={}",
        universe.step(),
        world.animals().len(),
        world.plants().len(),
    );
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
    world: nevo::run::UniverseView<nevo::universe::Universe>,
    control: Control,
    seed: u64,
    display: config::ViewerConfig,
    metrics_path: std::path::PathBuf,
    smoke: Option<smoke::SmokeCheck>,
    exit_requested: Arc<AtomicBool>,
) -> Result<(), String> {
    use std::sync::{Arc, Mutex};

    let result = Arc::new(Mutex::new(None));
    let result_out = result.clone();
    wgame::app::entry(async move || {
        let config = wgame::WindowConfig::default()
            .title("NEvo")
            .logical_size((1200.0, 900.0))
            .resizable(true)
            .vsync(display.vsync);
        let viewed = wgame::app_with_single_window(
            async move |window| {
                let options = viewer::Options {
                    labels: vec![Label::new(format!("Seed {seed}"))],
                    exit_requested: exit_requested.clone(),
                    interactive: smoke.is_none(),
                    metrics_path: Some(metrics_path.clone()),
                    initial_tab: match display.tab {
                        config::ViewerTab::World => viewer::Tab::World,
                        config::ViewerTab::Metrics => viewer::Tab::Metrics,
                    },
                    ..Default::default()
                };
                if let Some(mut check) = smoke.clone() {
                    let mut frame = 0;
                    let presented =
                        viewer::show_with(window, control.clone(), options, |drawing| {
                            check.command(frame, &control)?;
                            let snapshot = world.read();
                            snapshot.draw(drawing);
                            check.verify(frame, &snapshot, &control)?;
                            frame += 1;
                            Ok(if frame == 12 {
                                std::ops::ControlFlow::Break(())
                            } else {
                                std::ops::ControlFlow::Continue(())
                            })
                        })
                        .await?;
                    ensure!(
                        presented == 12,
                        "viewer closed before the smoke check finished"
                    );
                    println!("Verified 12 displayed snapshots and pause/step/pacing commands.");
                    Ok(())
                } else {
                    viewer::show(window, world.clone(), control.clone(), options).await
                }
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

fn execute(mut cli: Cli) -> Result<()> {
    if cli.latest {
        cli.directory = Some(persistence::latest_run(std::path::Path::new("runs"))?);
    }
    let mut options = cli.load_config(None)?;
    options.loop_options()?;
    let directory = RunDirectory::open(cli.directory.as_deref())?;
    // Reload the saved configuration under the writer lock in case another
    // invocation finished between argument parsing and acquiring the directory.
    if cli.directory.is_some() {
        options = cli.load_config(None)?;
    }
    let mut run = match directory.load_latest()? {
        Some(checkpoint) => {
            options = cli.load_config(Some(&checkpoint.config))?;
            ensure!(
                options.ecosystem.same_model(&checkpoint.config.ecosystem)
                    && options.seed == checkpoint.config.seed,
                "cannot change the seed or ecosystem model parameters of an existing run"
            );
            let run = checkpoint
                .state
                .restore(options.ecosystem.clone())
                .context("restoring checkpoint")?;
            eprintln!(
                "resuming={} tick={}",
                directory.path().display(),
                run.universe().read().step()
            );
            run
        }
        None => {
            ensure!(!cli.latest, "selected run no longer contains a checkpoint");
            directory.check_new()?;
            options.seed = Some(options.resolved_seed()?);
            Run::new(options.ecosystem.clone(), options.seed.unwrap())?
        }
    };
    let limits = options.loop_options()?;
    let seed = options
        .seed
        .expect("new and restored runs have a resolved seed");
    // No configuration is overwritten until the checkpoint is decoded and validated.
    let start_tick = run.universe().read().step();
    let mut metrics = MetricsLog::new(directory.path(), &options, start_tick)?;
    directory.save_config(&options)?;
    #[cfg(feature = "viewer")]
    let metrics_path = directory.path().join(nevo::metrics::log::FILE_NAME);
    eprintln!("run={}", directory.path().display());
    println!(
        "seed={} ticks={:?} interval={:?}",
        seed, limits.ticks, limits.interval,
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
    let world = run.universe().clone();
    #[cfg(feature = "viewer")]
    let smoke = options.smoke.then(|| smoke::SmokeCheck::new(run.clone()));
    let worker_options = options.clone();
    let worker_exit = Arc::clone(&exit_requested);
    let thread = thread::spawn(move || {
        let options = worker_options;
        print_progress(&run);
        let mut last_report = Instant::now();
        let mut save_error = None;
        let mut metrics_error = None;
        let stop = run.execute(limits, commands, |state, report| {
            if report.tick <= options.trace_ticks {
                print_trace(report);
            }
            if last_report.elapsed() >= Duration::from_secs(2) {
                print_progress(state);
                last_report = Instant::now();
            }
            if let Some(metrics) = &mut metrics
                && let Err(error) = metrics.record(state, false)
            {
                metrics_error = Some(error);
                return true;
            }
            if options.checkpoint_every != 0
                && report.tick.is_multiple_of(options.checkpoint_every)
                && let Err(error) = directory.save(state, &options)
            {
                save_error = Some(error);
                return true;
            }
            false
        });
        print_progress(&run);
        if let Ok(reason) = &stop {
            println!("stop={reason:?}");
        }
        let saved = match save_error {
            Some(error) => Err(error),
            None => directory.save(&run, &options),
        };
        let logged = match metrics_error {
            Some(error) => Err(error),
            None => match &mut metrics {
                Some(metrics) => metrics.finish(&run),
                None => Ok(()),
            },
        };
        if saved.is_err() || logged.is_err() || stop.is_err() {
            worker_exit.store(true, Ordering::Relaxed);
        }
        saved?;
        logged?;
        stop
    });
    let worker = Worker {
        control: control.clone(),
        thread: Some(thread),
    };
    #[cfg(feature = "viewer")]
    let viewed = if options.viewer.enable {
        let result = attach_viewer(
            world,
            control.clone(),
            seed,
            options.viewer,
            metrics_path,
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

pub fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(options) => options,
        Err(error) => {
            error.exit();
        }
    };
    match execute(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
