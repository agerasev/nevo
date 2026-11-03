#![forbid(unsafe_code)]

mod playback;
mod smoke;
mod viewer;

use std::{env, process::ExitCode};

use nevo::foraging::Scenario;

const HELP: &str = "\
Usage: nevo [--scenario baseline|debug] [--seed N] [--paused] [--smoke]

Open the foraging viewer. Defaults to the baseline scenario and seed 1.
Pause or resume with Space and advance one tick with Right Arrow when the arena
has focus. The toolbar provides the same controls. --paused opens at tick zero.
--smoke presents twelve frames and checks the displayed run against headless.";

struct Options {
    scenario: Scenario,
    seed: u64,
    paused: bool,
    smoke: bool,
}

impl Options {
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Self {
            scenario: Scenario::Baseline,
            seed: 1,
            paused: false,
            smoke: false,
        };
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--scenario" => {
                    options.scenario = args.next().ok_or("--scenario needs a name")?.parse()?;
                }
                "--seed" => {
                    let value = args.next().ok_or("--seed needs a number")?;
                    options.seed = value
                        .parse()
                        .map_err(|_| format!("invalid seed '{value}'"))?;
                }
                "--paused" => options.paused = true,
                "--smoke" => options.smoke = true,
                _ => return Err(format!("unknown option '{flag}'")),
            }
        }
        Ok(options)
    }
}

fn main() -> ExitCode {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.iter().any(|s| s == "--help" || s == "-h") {
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
    wgame::app::entry(async move || {
        let config = wgame::WindowConfig::default()
            .title("NEvo · Foraging")
            .logical_size((1200.0, 900.0))
            .resizable(true)
            .vsync(true);
        wgame::app_with_single_window(
            async move |window| {
                viewer::show(
                    window,
                    options.scenario,
                    options.seed,
                    options.paused,
                    options.smoke,
                )
                .await
            },
            config,
        )
        .await
    });
    ExitCode::SUCCESS
}
