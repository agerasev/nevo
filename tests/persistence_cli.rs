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

use nevo::{
    metrics::log::{DIRECTORY_NAME, FILE_NAME, Record, RunReader as Reader, Sample},
    universe::Universe,
};
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

    fn metrics_path(&self, run: &str) -> PathBuf {
        self.0
            .join(run)
            .join(DIRECTORY_NAME)
            .join("00000000000000000001.cborseq")
    }

    fn metrics(&self, run: &str) -> Vec<Record> {
        let mut reader = Reader::new(self.0.join(run));
        let records = drain_metrics(&mut reader);
        assert!(
            !reader.has_pending_record(),
            "orderly exit left an incomplete record"
        );
        records
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

#[derive(Debug, Deserialize)]
struct CheckpointConfig {
    ecosystem: nevo::ecosystem::Config,
}

#[derive(Debug, Deserialize)]
struct Checkpoint {
    config: CheckpointConfig,
    state: Universe,
}

fn read_checkpoint(path: &Path) -> Checkpoint {
    ciborium::from_reader(fs::File::open(path).unwrap()).unwrap()
}

fn checkpoint_tick(path: &Path) -> u64 {
    path.file_stem().unwrap().to_str().unwrap().parse().unwrap()
}

#[test]
fn a_new_process_resumes_the_complete_seeded_state() {
    let directory = Directory::new();
    let settings = directory.overlay("model.toml", "[ecosystem.genetics.initialization]\nhidden_units = 6\n[ecosystem.animal.body]\nmovement_cost = 0.003\nmovement_energy_power = 1.0\n");
    for (name, ticks) in [("full", "6"), ("split", "3")] {
        directory.success(&[
            name,
            "-c",
            &support::preset("debug"),
            "-c",
            &settings,
            "--ticks",
            ticks,
        ]);
    }
    // Resume without the original presets or initialization arguments.
    directory.success(&["split"]);
    let full = directory.latest("full");
    let split = directory.latest("split");
    assert_eq!(split.state.step(), 6);
    assert_eq!(full.state, split.state);
    assert_eq!(full.config.ecosystem, split.config.ecosystem);
    assert_eq!(
        split.config.ecosystem.genetics.initialization.hidden_units,
        6
    );
}

fn drain_metrics(reader: &mut Reader) -> Vec<Record> {
    let mut records = Vec::new();
    loop {
        let new = reader.poll().unwrap();
        if new.is_empty() {
            return records;
        }
        records.extend(new);
    }
}

fn samples(records: &[Record]) -> Vec<&Sample> {
    records
        .iter()
        .filter_map(|record| match record {
            Record::Sample(sample) => Some(sample),
            Record::Session(_) => None,
        })
        .collect()
}

fn metric_values(sample: &Sample) -> serde_json::Value {
    sample.values.deserialized().unwrap()
}

#[test]
fn invocation_files_record_full_partial_and_zero_tick_windows() {
    let directory = Directory::new();
    let window = directory.overlay("windows.toml", "[ecosystem.metrics]\nwindow_ticks = 3\n");
    for (run, ticks) in [("partial", "8"), ("complete", "6"), ("zero", "0")] {
        directory.success(&[
            run,
            "-c",
            &support::preset("debug"),
            "-c",
            &window,
            "--ticks",
            ticks,
        ]);
        assert!(directory.metrics_path(run).is_file());
        assert!(!directory.0.join(run).join(FILE_NAME).exists());
    }
    let records = directory.metrics("partial");
    let points = samples(&records);
    assert_eq!(points.len(), 3);
    for (sample, (start, end, count, complete)) in
        points
            .iter()
            .zip([(0, 3, 3, true), (3, 6, 3, true), (6, 8, 2, false)])
    {
        let v = metric_values(sample);
        assert_eq!(v["window"]["start_tick"], start);
        assert_eq!(v["window"]["end_tick"], end);
        assert_eq!(v["window"]["samples"], count);
        assert_eq!(v["window"]["complete"], complete);
    }
    assert_eq!(samples(&directory.metrics("complete")).len(), 2);
    assert_eq!(directory.metrics("zero").len(), 1);
    directory.success(&[
        "disabled",
        "-c",
        &support::preset("debug"),
        "-c",
        &directory.overlay("off.toml", "[ecosystem.metrics]\nenabled = false\n"),
        "--ticks",
        "8",
    ]);
    assert!(!directory.0.join("disabled/metrics").exists());
}

#[test]
fn periodic_and_exit_saves_obey_retention() {
    let directory = Directory::new();
    let policy = directory.overlay("policy.toml", "checkpoint_every = 2\nkeep_checkpoints = 2");
    directory.success(&["saved", "-c", &policy, "--ticks", "5"]);
    let ticks = || {
        directory
            .checkpoints("saved")
            .iter()
            .map(|p| checkpoint_tick(p))
            .collect::<Vec<_>>()
    };
    assert_eq!(ticks(), [4, 5]);
    directory.success(&["saved", "--ticks", "0"]);
    assert_eq!(ticks(), [4, 5]);
}

#[test]
fn a_zero_tick_run_saves_and_can_resume() {
    let directory = Directory::new();
    let exit_only = directory.overlay("exit.toml", "checkpoint_every = 0");
    directory.success(&["zero", "-c", &exit_only, "--ticks", "0"]);
    assert_eq!(directory.latest("zero").state.step(), 0);
    directory.success(&["zero", "--ticks", "1"]);
    assert_eq!(directory.latest("zero").state.step(), 1);
}

#[test]
fn latest_fails_on_corruption_instead_of_falling_back() {
    let directory = Directory::new();
    directory.success(&["runs/older", "--ticks", "0"]);
    directory.success(&["runs/newer", "--ticks", "1"]);
    directory.saved_at("runs/older", 100);
    directory.saved_at("runs/newer", 200);
    let path = directory.checkpoints("runs/newer").pop().unwrap();
    fs::write(&path, b"broken").unwrap();
    let output = directory.run(&["--latest", "--ticks", "1"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reading checkpoint"));
    assert_eq!(fs::read(path).unwrap(), b"broken");
    assert_eq!(directory.latest("runs/older").state.step(), 0);
}

#[test]
fn a_model_override_cannot_overwrite_the_saved_run() {
    let directory = Directory::new();
    directory.success(&["saved", "--seed", "1", "--ticks", "1"]);
    let config_path = directory.0.join("saved/config.toml");
    let checkpoint_path = directory.checkpoints("saved").pop().unwrap();
    let paths = [
        config_path,
        checkpoint_path,
        directory.metrics_path("saved"),
    ];
    let before: Vec<_> = paths.iter().map(|p| fs::read(p).unwrap()).collect();
    let change = directory.overlay("model.toml", "[ecosystem.world]\nwidth = 100.0");
    let output = directory.run(&["saved", "-c", &change, "--ticks", "0"]);
    assert!(!output.status.success());
    for (path, bytes) in paths.iter().zip(before) {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
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
    // Orderly shutdown released the writer lock.
    directory.success(&["live", "--ticks", "1", "--tick-hz", "0"]);
    assert_eq!(
        directory.latest("live").state.step(),
        saved.state.step() + 1
    );
}

#[test]
fn resume_starts_a_new_metrics_file_without_modifying_the_previous_one() {
    let directory = Directory::new();
    directory.success(&["saved", "--ticks", "1"]);
    let first = directory.metrics_path("saved");
    let bytes = fs::read(&first).unwrap();
    directory.success(&["saved", "--ticks", "1"]);
    assert_eq!(fs::read(first).unwrap(), bytes);
    assert_eq!(
        fs::read_dir(directory.0.join("saved/metrics"))
            .unwrap()
            .count(),
        2
    );
}
