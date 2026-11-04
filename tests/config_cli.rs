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
fn toml_and_cli_run_the_same_seeded_experiment() {
    let file = ConfigFile::new("scenario = 'debug'\nseed = 7\nticks = 3\nhz = 0\n");
    let from_file = success(&["--config", file.path()]);
    let from_cli = success(&["--scenario", "debug", "--seed", "7", "--ticks", "3"]);
    assert_eq!(from_file, from_cli);
    assert!(from_cli.contains("interval=0ns"));
    assert!(from_cli.contains("stop=TickLimit"));
}

#[test]
fn cli_overrides_file_values_including_true_booleans() {
    let file = ConfigFile::new(
        "scenario = 'baseline'\nseed = 1\nticks = 0\nhz = 25\nviewer = true\npaused = true\n",
    );
    let configured = success(&[
        &format!("--config={}", file.path()),
        "--scenario",
        "debug",
        "--seed",
        "17",
        "--ticks",
        "2",
        "--hz",
        "0",
        "--viewer=false",
        "--paused=false",
    ]);
    let explicit = success(&["--scenario", "debug", "--seed", "17", "--ticks", "2"]);
    assert_eq!(configured, explicit);
}

#[test]
fn entropy_seed_is_reported_and_can_replay_the_run() {
    let original = success(&["--scenario", "debug", "--ticks", "4"]);
    let seed = original
        .split_whitespace()
        .find_map(|field| field.strip_prefix("seed="))
        .expect("resolved seed in startup output");
    seed.parse::<u64>().unwrap();
    let replay = success(&["--scenario", "debug", "--ticks", "4", "--seed", seed]);
    assert_eq!(original, replay);
}

#[test]
fn quoted_seed_supports_the_full_u64_range() {
    let file = ConfigFile::new("scenario = 'debug'\nseed = '18446744073709551615'\nticks = 2\n");
    assert_eq!(
        success(&["--config", file.path()]),
        success(&[
            "--scenario",
            "debug",
            "--seed",
            "18446744073709551615",
            "--ticks",
            "2"
        ])
    );
}

#[test]
fn invalid_configuration_and_removed_options_are_rejected() {
    for content in [
        "unknown_key = true",
        "seconds = 1",
        "focal = 1",
        "hz = -1",
        "hz = nan",
        "seed = -1",
        "ticks = 'many'",
        "scenario = 'missing'",
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
    for option in ["--seconds", "--focal"] {
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
}
