use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct ConfigFile {
    directory: PathBuf,
    path: PathBuf,
}

impl ConfigFile {
    fn new(content: &str) -> Self {
        let directory = loop {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("nevo-config-test-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        };
        let path = directory.join("experiment config.toml");
        fs::write(&path, content).unwrap();
        Self { directory, path }
    }

    fn path(&self) -> &str {
        self.path.to_str().unwrap()
    }
}

impl Drop for ConfigFile {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn run(args: &[&str]) -> Output {
    // Each CLI invocation owns its default runs/ directory.
    let working = ConfigFile::new("");
    let mut child = Command::new(env!("CARGO_BIN_EXE_nevo"))
        .args(args)
        .current_dir(&working.directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("command did not finish: {args:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}

fn success(args: &[&str]) -> String {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn run_shortcuts_and_files_use_the_same_config() {
    let model = "trace_ticks = 3\n[ecosystem]\ninitial_animals = 2\nplants = 7\nminimum_population = 1\nimmigration.energy = 90.0\nimmigration.position = [10.0, 20.0]\nplant.initial_position = [-20.0, 12.0]\n";
    let complete = ConfigFile::new(&format!("seed = 7\nticks = 3\n{model}"));
    let partial = ConfigFile::new(model);
    let from_file = success(&["--config", complete.path()]);
    let from_cli = success(&["--config", partial.path(), "--seed", "7", "--ticks", "3"]);
    assert_eq!(from_file, from_cli);
    assert!(from_cli.contains("interval=0ns"));
    assert!(from_cli.contains("stop=TickLimit"));
    assert!(from_cli.contains("tick=0 animals=2 plants=7"));
}

#[test]
fn explicit_shortcuts_override_files_including_nested_viewer_enable() {
    let file = ConfigFile::new(
        "seed = 1\nticks = 0\ntick_hz = 25\npaused = true\n[viewer]\nenable = true\nvsync = false\n[ecosystem]\ninitial_animals = 3\n",
    );
    let configured = success(&[
        &format!("--config={}", file.path()),
        "--seed",
        "17",
        "--ticks",
        "2",
        "--tick-hz",
        "0",
        "--viewer=false",
        "--paused=false",
    ]);
    let model = ConfigFile::new("[ecosystem]\ninitial_animals = 3\n");
    let explicit = success(&["--config", model.path(), "--seed", "17", "--ticks", "2"]);
    assert_eq!(configured, explicit);
}

#[test]
fn entropy_seed_is_reported_and_can_replay_the_run() {
    let original = success(&["--ticks", "4"]);
    let seed = original
        .split_whitespace()
        .find_map(|field| field.strip_prefix("seed="))
        .expect("resolved seed in startup output");
    seed.parse::<u64>().unwrap();
    let replay = success(&["--ticks", "4", "--seed", seed]);
    assert_eq!(original, replay);
}

#[test]
fn quoted_seed_supports_the_full_u64_range() {
    let file = ConfigFile::new("seed = '18446744073709551615'\nticks = 2\n");
    assert_eq!(
        success(&["--config", file.path()]),
        success(&["--seed", "18446744073709551615", "--ticks", "2"])
    );
}

#[test]
fn invalid_configuration_and_removed_options_are_rejected() {
    for content in [
        "unknown_key = true",
        "seconds = 1",
        "focal = 1",
        "tick_hz = -1",
        "tick_hz = nan",
        "hz = 25",
        "viewer = true",
        "[viewer]\nenable = 1",
        "[viewer]\nvsync = 1",
        "[viewer]\nunknown = true",
        "seed = -1",
        "ticks = 'many'",
        "scenario = 'debug'",
        "[ecosystem]\nunknown = 1",
        "[ecosystem]\nplants = -1",
        "[ecosystem]\nimmigration.energy = nan",
        "[ecosystem]\nimmigration.position = [nan, 0.0]",
        "[ecosystem]\nplant.initial_position = [631.0, 0.0]",
        "[ecosystem]\nplant.initial_position = [0.0]",
        "[ecosystem]\ninitial_animal_energy = 100.0",
        "[ecosystem]\ninitial_animal_position = 'random'",
        "[ecosystem]\ninitial_plant_position = 'random'",
        "[ecosystem.world]\nwidth = 0.0",
        "[ecosystem.world]\nheight = inf",
        "[ecosystem.animal.body]\nupkeep = -1.0",
        "[ecosystem.animal.reproduction]\nchild_energy_fraction = 1.0",
        "[ecosystem.plant]\ngrowth = -1.0",
        "[ecosystem.genetics.initialization]\nhidden_units = 0",
        "[ecosystem.genetics.initialization]\nweight_min = 1.0\nweight_max = 0.0",
        "[ecosystem.genetics.mutation]\nmax_delta = -0.01",
        "not valid toml",
    ] {
        let bounded = if content.starts_with("ticks =") {
            content.to_owned()
        } else {
            format!("ticks = 0\n{content}")
        };
        let file = ConfigFile::new(&bounded);
        let output = run(&["--config", file.path()]);
        assert!(!output.status.success(), "accepted {content:?}");
        assert!(!output.stderr.is_empty());
    }
    for option in [
        "--seconds",
        "--focal",
        "--scenario",
        "--hz",
        "--ecosystem-initial-animals",
        "--viewer-vsync",
        "--viewer-enable",
        "--set",
        "--checkpoint-every",
        "--trace-ticks",
        "--smoke",
    ] {
        let output = run(&[option, "1", "--ticks", "0"]);
        assert!(!output.status.success(), "accepted {option}");
        assert!(String::from_utf8_lossy(&output.stderr).contains(option));
    }
}

#[test]
fn missing_config_fails_but_help_does_not_need_the_file() {
    let file = ConfigFile::new("");
    let missing = file.directory.join("missing.toml");
    let missing = missing.to_str().unwrap();
    let output = run(&["--config", missing, "--ticks", "0"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing.toml"));
    let help = success(&["--config", missing, "--help"]);
    assert!(help.contains("--config"));
    assert!(!help.contains("--seconds"));
    assert!(!help.contains("--focal"));
    assert!(!help.contains("--scenario"));
    assert!(!help.contains("--ecosystem-initial-animals"));
    assert!(!help.contains("--viewer-vsync"));
    assert!(!help.contains("--hz"));
    assert!(help.contains("--tick-hz"));
    assert!(help.contains("--viewer"));
    assert!(help.contains("--latest"));
}

#[test]
fn ordered_files_merge_nested_settings_with_cli_precedence() {
    let base = ConfigFile::new(
        "seed = 1\nticks = 2\ntrace_ticks = 2\n[ecosystem]\ninitial_animals = 1\nplants = 3\nminimum_population = 0\nimmigration.position = [10.0, 20.0]\n",
    );
    let overlay = ConfigFile::new(
        "seed = 7\n[ecosystem]\nplants = 5\nimmigration.position = [-30.0, 40.0]\n",
    );
    let layered = success(&[
        "--seed",
        "42",
        "--config",
        base.path(),
        "--config",
        overlay.path(),
    ]);
    let complete = ConfigFile::new(
        "seed = 42\nticks = 2\ntrace_ticks = 2\n[ecosystem]\ninitial_animals = 1\nplants = 5\nminimum_population = 0\nimmigration.position = [-30.0, 40.0]\n",
    );
    assert_eq!(layered, success(&["--config", complete.path()]));
    let reversed = success(&[
        "--config",
        overlay.path(),
        "--config",
        base.path(),
        "--seed",
        "42",
    ]);
    assert!(reversed.contains("tick=0 animals=1 plants=3"));
    let reset = ConfigFile::new("[ecosystem]\nimmigration.position = 'random'\n");
    let random = ConfigFile::new(
        "seed = 1\nticks = 2\ntrace_ticks = 2\n[ecosystem]\ninitial_animals = 1\nplants = 3\nminimum_population = 0\n",
    );
    assert_eq!(
        success(&["--config", base.path(), "--config", reset.path()]),
        success(&["--config", random.path()]),
    );
}

#[test]
fn every_config_file_is_checked_even_if_an_overlay_replaces_it() {
    let valid = ConfigFile::new("ticks = 0\n[ecosystem]\nplants = 5\n");
    for content in ["[ecosystem]\nplants = 'wrong'", "[ecosystem]\nunknown = 5"] {
        let invalid = ConfigFile::new(content);
        let output = run(&["--config", invalid.path(), "--config", valid.path()]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(invalid.path()));
    }
}
