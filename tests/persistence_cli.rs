#[allow(dead_code)]
mod support;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant, SystemTime},
};

use nevo::{run::Run, universe::Universe};
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

    fn saved_at(&self, run: &str, seconds: u64) {
        for checkpoint in self.checkpoints(run) {
            fs::OpenOptions::new()
                .write(true)
                .open(checkpoint)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
                .unwrap();
        }
    }

    fn overlay(&self, name: &str, content: &str) -> String {
        let path = self.0.join(name);
        fs::write(&path, content).unwrap();
        path.to_str().unwrap().to_owned()
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
    state: Universe,
}

fn read_checkpoint(path: &Path) -> Checkpoint {
    let checkpoint: Checkpoint = ciborium::from_reader(fs::File::open(path).unwrap()).unwrap();
    assert_eq!(checkpoint.format_version, 6);
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
    assert_eq!(full.state.step(), 256);
    assert_eq!(full.counts, split.counts);
    assert_eq!(full.state, split.state);
    let mut full = full.state.restore().unwrap();
    let mut split = split.state.restore().unwrap();
    for _ in 0..64 {
        assert_eq!(full.tick().unwrap(), split.tick().unwrap());
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
        "--config",
        &directory.overlay(
            "policy.toml",
            "checkpoint_every = 10\nkeep_checkpoints = 2\n",
        ),
    ]);
    let ticks = || {
        directory
            .checkpoints("retained")
            .iter()
            .map(|path| checkpoint_tick(path))
            .collect::<Vec<_>>()
    };
    assert_eq!(ticks(), vec![20, 25]);
    assert_eq!(directory.latest("retained").state.step(), 25);
    directory.success(&["retained", "--ticks", "6"]);
    assert_eq!(ticks(), vec![30, 31]);
    // Saving the same tick at exit replaces it; it does not consume another slot.
    directory.success(&[
        "retained",
        "--ticks",
        "0",
        "--config",
        &directory.overlay("retention.toml", "keep_checkpoints = 1\n"),
    ]);
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
        "--config",
        &support::preset("debug"),
        "--seed",
        "1",
        "--ticks",
        "7",
        "--config",
        &directory.overlay("exit-only.toml", "checkpoint_every = 0\n"),
    ]);
    assert_eq!(directory.checkpoints("exit-only").len(), 1);
    assert_eq!(directory.latest("exit-only").state.step(), 7);
    directory.success(&["exit-only", "--ticks", "0"]);
    assert_eq!(directory.checkpoints("exit-only").len(), 1);
    assert_eq!(directory.latest("exit-only").state.step(), 7);

    directory.success(&[
        "zero",
        "--config",
        &support::preset("debug"),
        "--seed",
        "7",
        "--ticks",
        "0",
    ]);
    assert_eq!(
        directory.latest("zero").state.restore().unwrap(),
        Run::new(support::ecosystem("debug"), 7).unwrap()
    );
    directory.success(&["zero", "--ticks", "3"]);
    let mut reference = Run::new(support::ecosystem("debug"), 7).unwrap();
    for _ in 0..3 {
        reference.tick().unwrap();
    }
    assert_eq!(directory.latest("zero").state.restore().unwrap(), reference);
}

#[test]
fn default_directory_records_entropy_and_full_range_seeds() {
    let directory = Directory::new();
    let output = directory.success(&["--ticks", "2"]);
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
    let name = run.file_name().unwrap().to_str().unwrap();
    assert_eq!(name.len(), 19);
    jiff::civil::DateTime::strptime("%Y-%m-%d_%H-%M-%S", name).unwrap();
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(run.join("config.toml")).unwrap()).unwrap();
    assert_eq!(
        config["seed"].as_str().unwrap().parse::<u64>().unwrap(),
        seed
    );
    directory.success(&[run.to_str().unwrap()]);
    assert_eq!(directory.latest(run.to_str().unwrap()).state.step(), 4);
    directory.success(&["maximum", "--seed", "18446744073709551615", "--ticks", "0"]);
    directory.success(&["maximum", "--ticks", "1"]);
    let mut reference = Run::new(support::ecosystem("baseline"), u64::MAX).unwrap();
    reference.tick().unwrap();
    assert_eq!(
        directory.latest("maximum").state.restore().unwrap(),
        reference
    );
}

#[test]
fn latest_resumes_by_save_time_with_saved_config_and_explicit_overrides() {
    let directory = Directory::new();
    directory.success(&["runs/z-older", "--seed", "1", "--ticks", "9"]);
    directory.success(&[
        "runs/a-newer",
        "--config",
        &support::preset("debug"),
        "--seed",
        "42",
        "--ticks",
        "2",
    ]);
    directory.saved_at("runs/z-older", 100);
    directory.saved_at("runs/a-newer", 200);
    // Unfinished saves, artifact folders, and runs outside runs/ are not candidates.
    fs::create_dir_all(directory.0.join("runs/artifacts/checkpoints")).unwrap();
    fs::write(
        directory
            .0
            .join("runs/artifacts/checkpoints/.00000000000000000003.cbor.tmp"),
        b"incomplete",
    )
    .unwrap();
    directory.success(&["outside", "--ticks", "0"]);
    directory.success(&[
        "--latest",
        "--config",
        &directory.overlay("override.toml", "ticks = 7\n[viewer]\nvsync = false\n"),
        "--ticks",
        "1",
    ]);
    let mut reference = Run::new(support::ecosystem("debug"), 42).unwrap();
    for _ in 0..3 {
        reference.tick().unwrap();
    }
    assert_eq!(
        directory.latest("runs/a-newer").state.restore().unwrap(),
        reference
    );
    assert_eq!(directory.latest("runs/z-older").state.step(), 9);
    let config = directory.config("runs/a-newer");
    assert_eq!(config["viewer"]["vsync"].as_bool(), Some(false));
    assert!(config.get("latest").is_none());

    // Saving an older run makes it the latest, irrespective of its creation/name.
    directory.success(&["runs/z-older", "--ticks", "0"]);
    directory.saved_at("runs/a-newer", 200);
    directory.success(&["--latest", "--ticks", "1"]);
    assert_eq!(directory.latest("runs/z-older").state.step(), 10);
    assert_eq!(directory.latest("runs/a-newer").state.step(), 3);
}

#[test]
fn latest_requires_a_saved_run_and_conflicts_with_a_directory() {
    let directory = Directory::new();
    let missing = directory.run(&["--latest", "--ticks", "0"]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("no saved runs"));
    assert!(!directory.0.join("runs").exists());

    fs::create_dir_all(directory.0.join("runs/unfinished/checkpoints")).unwrap();
    let empty = directory.run(&["--latest", "--ticks", "0"]);
    assert!(!empty.status.success());
    assert!(String::from_utf8_lossy(&empty.stderr).contains("no saved runs"));
    assert!(!directory.0.join("runs/unfinished/config.toml").exists());

    let conflict = directory.run(&["explicit", "--latest", "--ticks", "0"]);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("cannot be used with"));
    assert!(!directory.0.join("explicit").exists());
}

#[test]
fn latest_does_not_fall_back_when_selected_run_is_locked_or_corrupt() {
    let directory = Directory::new();
    directory.success(&["runs/older", "--seed", "1", "--ticks", "2"]);
    directory.success(&["runs/newer", "--seed", "42", "--ticks", "3"]);
    directory.saved_at("runs/older", 100);
    directory.saved_at("runs/newer", 200);
    let config_path = directory.0.join("runs/newer/config.toml");
    let config = fs::read(&config_path).unwrap();
    let checkpoint_path = directory.checkpoints("runs/newer").pop().unwrap();
    let checkpoint = fs::read(&checkpoint_path).unwrap();

    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(directory.0.join("runs/newer/.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let locked = directory.run(&["--latest", "--ticks", "1"]);
    assert!(!locked.status.success());
    assert!(String::from_utf8_lossy(&locked.stderr).contains("cannot lock run directory"));
    assert_eq!(fs::read(&checkpoint_path).unwrap(), checkpoint);
    drop(lock);

    fs::write(&checkpoint_path, b"broken checkpoint").unwrap();
    let corrupt = directory.run(&["--latest", "--ticks", "1"]);
    assert!(!corrupt.status.success());
    assert!(String::from_utf8_lossy(&corrupt.stderr).contains("reading checkpoint"));
    assert_eq!(fs::read(&config_path).unwrap(), config);
    assert_eq!(fs::read(&checkpoint_path).unwrap(), b"broken checkpoint");
    assert_eq!(directory.latest("runs/older").state.step(), 2);
}

#[test]
fn invalid_resume_never_overwrites_the_saved_run() {
    let directory = Directory::new();
    directory.success(&["saved", "--seed", "1", "--ticks", "3"]);
    let config_path = directory.0.join("saved/config.toml");
    let config = fs::read(&config_path).unwrap();
    let checkpoint_path = directory.checkpoints("saved").pop().unwrap();
    let checkpoint = fs::read(&checkpoint_path).unwrap();
    fs::write(
        directory.0.join("replacement.toml"),
        "seed = 999\nticks = 0\n",
    )
    .unwrap();
    for args in [
        vec!["saved", "--seed", "2"],
        vec!["saved", "--config", &support::preset("debug")],
        vec!["saved", "--config", "replacement.toml"],
        vec![
            "saved",
            "--config",
            &directory.overlay("invalid-policy.toml", "keep_checkpoints = 0\n"),
        ],
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
        "--config",
        &support::preset("debug"),
        "--seed",
        "42",
        "--ticks",
        "7",
        "--config",
        &directory.overlay(
            "policy.toml",
            "checkpoint_every = 3\nkeep_checkpoints = 2\n",
        ),
    ]);
    let config_path = directory.0.join("recover/config.toml");
    fs::remove_file(&config_path).unwrap();
    let partial = directory
        .0
        .join("recover/checkpoints/.00000000000000000009.cbor.tmp");
    fs::write(&partial, b"incomplete checkpoint").unwrap();
    fs::write(
        directory.0.join("replacement.toml"),
        "seed = 999\nticks = 0\n",
    )
    .unwrap();
    let saved_path = directory.checkpoints("recover").pop().unwrap();
    let saved = fs::read(&saved_path).unwrap();

    let rejected = directory.run(&["recover", "--config", "replacement.toml"]);
    assert!(!rejected.status.success());
    assert!(!config_path.exists());
    assert_eq!(fs::read(&saved_path).unwrap(), saved);

    fs::write(
        directory.0.join("replacement.toml"),
        "ticks = 5\ntick_hz = 0\n",
    )
    .unwrap();
    directory.success(&["recover", "--config", "replacement.toml"]);
    let mut reference = Run::new(support::ecosystem("debug"), 42).unwrap();
    for _ in 0..12 {
        reference.tick().unwrap();
    }
    assert_eq!(
        directory.latest("recover").state.restore().unwrap(),
        reference
    );
    let config = directory.config("recover");
    assert_eq!(config["ecosystem"]["initial_animals"].as_integer(), Some(1));
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
        "--tick-hz",
        "50",
        "--config",
        &directory.overlay("policy.toml", "checkpoint_every = 1\n"),
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
    assert!(saved.state.step() >= 1);
    let mut reference = Run::new(support::ecosystem("baseline"), 1).unwrap();
    for _ in 0..saved.state.step() {
        reference.tick().unwrap();
    }
    assert_eq!(saved.state.restore().unwrap(), reference);
    // The process released the OS lock; a subsequent invocation can continue.
    directory.success(&["live", "--ticks", "1", "--tick-hz", "0"]);
}
