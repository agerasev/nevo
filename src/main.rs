#![forbid(unsafe_code)]

mod config;

#[cfg(feature = "viewer")]
mod smoke;
#[cfg(feature = "viewer")]
mod viewer;

use std::{
    process::ExitCode,
    thread,
    time::{Duration, Instant},
};

use nevo::{
    foraging::{Event, Scenario, TickReport},
    run::{Control, Run, StopReason},
};

#[cfg(feature = "viewer")]
use nevo::run::Command;

use config::Options;

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
    let limits = options
        .loop_options()
        .map_err(|error| format!("{error:#}"))?;
    let seed = options
        .resolved_seed()
        .map_err(|error| format!("{error:#}"))?;
    println!(
        "scenario={:?} seed={} ticks={:?} interval={:?}",
        options.scenario, seed, limits.ticks, limits.interval,
    );
    let mut run = Run::new(options.scenario, seed);
    let (control, commands) = Control::channel(options.paused || options.smoke);
    #[cfg(feature = "viewer")]
    let world = run.world().clone();
    let thread = thread::spawn(move || {
        let mut counts = Counts::default();
        counts.print(&run);
        let mut last_report = Instant::now();
        let stop = run.execute(limits, commands, |state, report| {
            counts.record(report);
            if options.scenario == Scenario::Debug && report.tick <= 3 {
                print_trace(report);
            }
            if last_report.elapsed() >= Duration::from_secs(2) {
                counts.print(state);
                last_report = Instant::now();
            }
            false
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
            seed,
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
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
