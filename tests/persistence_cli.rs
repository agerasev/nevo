use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

use nevo::{environment::ecosystem::Scenario, run::Run, run::checkpoint::Snapshot};
use serde::Deserialize;

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("nevo-persistence-test-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        }
    }

    fn spawn(&self, args: &[&str]) -> Running {
        Running(Some(
            Command::new(env!("CARGO_BIN_EXE_nevo"))
                .args(args)
                .current_dir(&self.0)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        ))
    }

    fn run(&self, args: &[&str]) -> Output {
        self.spawn(args).wait()
    }

    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn checkpoints(&self, run: &str) -> Vec<PathBuf> {
        let mut files: Vec<_> = fs::read_dir(self.0.join(run).join("checkpoints"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "cbor")
            })
            .collect();
        files.sort();
        files
    }

    fn latest(&self, run: &str) -> Checkpoint {
        read_checkpoint(self.checkpoints(run).last().expect("checkpoint exists"))
    }

    fn config(&self, run: &str) -> toml::Value {
        let text = fs::read_to_string(self.0.join(run).join("config.toml")).unwrap();
        toml::from_str(&text).unwrap()
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

struct Running(Option<Child>);

impl Running {
    fn wait(mut self) -> Output {
        let start = Instant::now();
        while self.0.as_mut().unwrap().try_wait().unwrap().is_none() {
            assert!(start.elapsed() < Duration::from_secs(15), "CLI timed out");
            thread::sleep(Duration::from_millis(10));
        }
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
struct Counts {
    meals: u64,
    births: u64,
    deaths: u64,
    immigrants: u64,
}

#[derive(Debug, Deserialize)]
struct Checkpoint {
    format_version: u32,
    counts: Counts,
    state: Snapshot,
}

fn read_checkpoint(path: &Path) -> Checkpoint {
    let checkpoint: Checkpoint = ciborium::from_reader(fs::File::open(path).unwrap()).unwrap();
    assert_eq!(checkpoint.format_version, 1);
    checkpoint
}

fn checkpoint_tick(path: &Path) -> u64 {
    path.file_stem().unwrap().to_str().unwrap().parse().unwrap()
}

#[test]
fn a_new_process_resumes_the_complete_seeded_state_and_counts() {
    let directory = Directory::new();
    directory.success(&["full", "--seed", "1", "--ticks", "256"]);
    directory.success(&["split", "--seed", "1", "--ticks", "128"]);
    // No --config or initialization arguments: the directory supplies them all.
    let resumed_output = directory.success(&["split"]);
    assert!(resumed_output.contains("tick=256 "));
    let full = directory.latest("full");
    let split = directory.latest("split");
    assert_eq!(full.state.tick(), 256);
    assert_eq!(full.counts, split.counts);
    assert_eq!(full.state, split.state);
    let mut full = full.state.restore().unwrap();
    let mut split = split.state.restore().unwrap();
    for _ in 0..64 {
        assert_eq!(full.tick(), split.tick());
    }
    assert_eq!(full, split);
}

#[test]
fn periodic_and_exit_saves_obey_retention_across_resumes() {
    let directory = Directory::new();
    directory.success(&[
        "retained",
        "--seed",
        "42",
        "--ticks",
        "25",
        "--checkpoint-every",
        "10",
        "--keep-checkpoints",
        "2",
    ]);
    let ticks = || {
        directory
            .checkpoints("retained")
            .iter()
            .map(|path| checkpoint_tick(path))
            .collect::<Vec<_>>()
    };
    assert_eq!(ticks(), vec![20, 25]);
    assert_eq!(directory.latest("retained").state.tick(), 25);
    directory.success(&["retained", "--ticks", "6"]);
    assert_eq!(ticks(), vec![30, 31]);
    // Saving the same tick at exit replaces it; it does not consume another slot.
    directory.success(&["retained", "--ticks", "0", "--keep-checkpoints", "1"]);
    assert_eq!(ticks(), vec![31]);
    assert_eq!(
        directory.config("retained")["keep_checkpoints"].as_integer(),
        Some(1)
    );
    assert_eq!(directory.config("retained")["ticks"].as_integer(), Some(0));
}

#[test]
fn exit_only_and_zero_tick_runs_are_resumable() {
    let directory = Directory::new();
    directory.success(&[
        "exit-only",
        "--scenario",
        "debug",
        "--seed",
        "1",
        "--ticks",
        "7",
        "--checkpoint-every",
        "0",
    ]);
    assert_eq!(directory.checkpoints("exit-only").len(), 1);
    assert_eq!(directory.latest("exit-only").state.tick(), 7);
    directory.success(&["exit-only", "--ticks", "0"]);
    assert_eq!(directory.checkpoints("exit-only").len(), 1);
    assert_eq!(directory.latest("exit-only").state.tick(), 7);

    directory.success(&["zero", "--scenario", "debug", "--seed", "7", "--ticks", "0"]);
    assert_eq!(
        directory.latest("zero").state.restore().unwrap(),
        Run::new(Scenario::Debug, 7)
    );
    directory.success(&["zero", "--ticks", "3"]);
    let mut reference = Run::new(Scenario::Debug, 7);
    for _ in 0..3 {
        reference.tick();
    }
    assert_eq!(directory.latest("zero").state.restore().unwrap(), reference);
}

#[test]
fn default_directory_records_entropy_and_full_range_seeds() {
    let directory = Directory::new();
    let output = directory.success(&["--scenario", "debug", "--ticks", "2"]);
    let seed: u64 = output
        .split_whitespace()
        .find_map(|field| field.strip_prefix("seed="))
        .unwrap()
        .parse()
        .unwrap();
    let run = fs::read_dir(directory.0.join("runs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(run.join("config.toml")).unwrap()).unwrap();
    assert_eq!(
        config["seed"].as_str().unwrap().parse::<u64>().unwrap(),
        seed
    );
    directory.success(&[run.to_str().unwrap()]);
    assert_eq!(directory.latest(run.to_str().unwrap()).state.tick(), 4);
    directory.success(&["maximum", "--seed", "18446744073709551615", "--ticks", "0"]);
    directory.success(&["maximum", "--ticks", "1"]);
    let mut reference = Run::new(Scenario::Baseline, u64::MAX);
    reference.tick();
    assert_eq!(
        directory.latest("maximum").state.restore().unwrap(),
        reference
    );
}

#[test]
fn invalid_resume_never_overwrites_the_saved_run() {
    let directory = Directory::new();
    directory.success(&["saved", "--seed", "1", "--ticks", "3"]);
    let config_path = directory.0.join("saved/config.toml");
    let config = fs::read(&config_path).unwrap();
    let checkpoint_path = directory.checkpoints("saved").pop().unwrap();
    let checkpoint = fs::read(&checkpoint_path).unwrap();
    fs::write(directory.0.join("replacement.toml"), "ticks = 0\n").unwrap();
    for args in [
        vec!["saved", "--seed", "2"],
        vec!["saved", "--scenario", "debug"],
        vec!["saved", "--config", "replacement.toml"],
        vec!["saved", "--keep-checkpoints", "0"],
    ] {
        let output = directory.run(&args);
        assert!(!output.status.success(), "accepted {args:?}");
        assert_eq!(fs::read(&config_path).unwrap(), config);
        assert_eq!(fs::read(&checkpoint_path).unwrap(), checkpoint);
    }
    fs::write(&checkpoint_path, &checkpoint[..checkpoint.len() / 2]).unwrap();
    let broken = fs::read(&checkpoint_path).unwrap();
    let output = directory.run(&["saved"]);
    assert!(!output.status.success());
    assert_eq!(fs::read(&config_path).unwrap(), config);
    assert_eq!(fs::read(&checkpoint_path).unwrap(), broken);
}

#[test]
fn checkpoint_recovers_missing_config_and_ignores_an_interrupted_write() {
    let directory = Directory::new();
    directory.success(&[
        "recover",
        "--scenario",
        "debug",
        "--seed",
        "42",
        "--ticks",
        "7",
        "--checkpoint-every",
        "3",
        "--keep-checkpoints",
        "2",
    ]);
    let config_path = directory.0.join("recover/config.toml");
    fs::remove_file(&config_path).unwrap();
    let partial = directory
        .0
        .join("recover/checkpoints/.00000000000000000009.cbor.tmp");
    fs::write(&partial, b"incomplete checkpoint").unwrap();
    fs::write(directory.0.join("replacement.toml"), "ticks = 0\n").unwrap();
    let saved_path = directory.checkpoints("recover").pop().unwrap();
    let saved = fs::read(&saved_path).unwrap();

    let rejected = directory.run(&["recover", "--config", "replacement.toml"]);
    assert!(!rejected.status.success());
    assert!(!config_path.exists());
    assert_eq!(fs::read(&saved_path).unwrap(), saved);

    directory.success(&["recover", "--ticks", "5"]);
    let mut reference = Run::new(Scenario::Debug, 42);
    for _ in 0..12 {
        reference.tick();
    }
    assert_eq!(
        directory.latest("recover").state.restore().unwrap(),
        reference
    );
    let config = directory.config("recover");
    assert_eq!(config["scenario"].as_str(), Some("debug"));
    assert_eq!(config["seed"].as_str(), Some("42"));
    assert_eq!(config["ticks"].as_integer(), Some(5));
    assert_eq!(config["checkpoint_every"].as_integer(), Some(3));
    assert_eq!(config["keep_checkpoints"].as_integer(), Some(2));
    assert!(!partial.exists());
    assert_eq!(
        directory
            .checkpoints("recover")
            .iter()
            .map(|path| checkpoint_tick(path))
            .collect::<Vec<_>>(),
        vec![9, 12]
    );
}

#[cfg(unix)]
#[test]
fn a_live_run_rejects_another_writer_and_saves_on_ctrl_c() {
    let directory = Directory::new();
    let child = directory.spawn(&[
        "live",
        "--seed",
        "1",
        "--hz",
        "50",
        "--checkpoint-every",
        "1",
    ]);
    let start = Instant::now();
    while !fs::read_dir(directory.0.join("live/checkpoints")).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "cbor")
        })
    }) {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "run never saved its first tick"
        );
        thread::sleep(Duration::from_millis(5));
    }
    let blocked = directory.run(&["live", "--ticks", "0"]);
    assert!(!blocked.status.success());
    assert!(
        Command::new("kill")
            .args(["-INT", &child.0.as_ref().unwrap().id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let stopped = child.wait();
    assert!(
        stopped.status.success(),
        "{}",
        String::from_utf8_lossy(&stopped.stderr)
    );
    assert!(String::from_utf8_lossy(&stopped.stdout).contains("stop=Requested"));
    let saved = directory.latest("live");
    assert!(saved.state.tick() >= 1);
    let mut reference = Run::new(Scenario::Baseline, 1);
    for _ in 0..saved.state.tick() {
        reference.tick();
    }
    assert_eq!(saved.state.restore().unwrap(), reference);
    // The process released the OS lock; a subsequent invocation can continue.
    directory.success(&["live", "--ticks", "1", "--hz", "0"]);
}
