#[allow(dead_code)]
mod support;

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant, SystemTime},
};

use nevo::{
    metrics::Metrics,
    metrics::log::{DIRECTORY_NAME, FILE_NAME, Record, RunReader as Reader, Sample},
    run::Run,
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

    fn config(&self, run: &str) -> toml::Value {
        let text = fs::read_to_string(self.0.join(run).join("config.toml")).unwrap();
        toml::from_str(&text).unwrap()
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
    format_version: u32,
    config: CheckpointConfig,
    state: Universe,
}

impl Checkpoint {
    fn restore(self) -> Run {
        self.state.restore(self.config.ecosystem).unwrap()
    }
}

fn read_checkpoint(path: &Path) -> Checkpoint {
    let checkpoint: Checkpoint = ciborium::from_reader(fs::File::open(path).unwrap()).unwrap();
    assert_eq!(checkpoint.format_version, 11);
    checkpoint
}

fn checkpoint_tick(path: &Path) -> u64 {
    path.file_stem().unwrap().to_str().unwrap().parse().unwrap()
}

#[test]
fn a_new_process_resumes_the_complete_seeded_state() {
    let directory = Directory::new();
    directory.success(&["full", "--seed", "1", "--ticks", "256"]);
    directory.success(&["split", "--seed", "1", "--ticks", "128"]);
    // No --config or initialization arguments: the directory supplies them all.
    let resumed_output = directory.success(&["split"]);
    assert!(resumed_output.contains("tick=256 "));
    let full = directory.latest("full");
    let split = directory.latest("split");
    assert_eq!(full.state.step(), 256);
    assert_eq!(full.state, split.state);
    let mut full = full.restore();
    let mut split = split.restore();
    for _ in 0..64 {
        assert_eq!(full.tick().unwrap(), split.tick().unwrap());
    }
    assert_eq!(full, split);
}

#[test]
fn configured_physics_and_brain_shape_survive_process_resume() {
    let directory = Directory::new();
    let settings = directory.overlay(
        "parameters.toml",
        r#"
[ecosystem]
initial_animals = 3
plants = 9
minimum_population = 3
[ecosystem.world]
width = 80.0
height = 60.0
[ecosystem.immigration]
energy = 0.175
position = [4.0, 5.0]
[ecosystem.animal.body]
upkeep = 0.00375
movement_cost = 0.0009
movement_energy_power = 1.0
max_speed = 30.0
movement_time_step = 0.2
movement_dead_zone = 0.05
radius_scale = 5.0
[ecosystem.animal.reproduction]
threshold = 0.15
child_energy_fraction = 0.3
separation_scale = 0.4
[ecosystem.animal.feeding]
efficiency = 0.2
contact_scale = 0.9
[ecosystem.plant]
initial_energy = 0.2
replacement_energy = 0.0125
growth = 0.001875
max_energy = 0.25
radius_scale = 6.0
initial_position = [-2.0, -3.0]
replacement_position = "random"
[ecosystem.sensing]
distance_scale = 0.75
softening = 0.02
gradient_dead_zone = 0.005
[ecosystem.genetics.initialization]
hidden_units = 6
weight_min = -0.3
weight_max = 0.25
[ecosystem.genetics.mutation]
max_delta = 0.04
"#,
    );
    directory.success(&[
        "full", "--config", &settings, "--seed", "17", "--ticks", "0",
    ]);
    let initial = directory.latest("full");
    for animal in initial.state.world().animals().values() {
        assert_eq!(animal.body.position, glam::DVec2::new(4.0, 5.0));
        assert_eq!(animal.body.energy, 0.175);
        assert_eq!(animal.body.radius(), 0.25 * 70.0_f64.sqrt());
        assert_eq!(animal.mind.brain.genome.hidden_units, 6);
        assert_eq!(animal.mind.brain.genome.weights.len(), 81);
        assert_eq!(animal.mind.brain.memory, vec![0.0; 6]);
        assert!(
            animal
                .mind
                .brain
                .genome
                .weights
                .iter()
                .all(|w| (-0.3..0.25).contains(w))
        );
    }
    for plant in initial.state.world().plants() {
        assert_eq!(plant.position, glam::DVec2::new(-2.0, -3.0));
        assert_eq!(plant.energy, 0.2);
        assert_eq!(plant.radius(), 0.3 * 80.0_f64.sqrt());
    }
    directory.success(&["full", "--ticks", "64"]);
    directory.success(&[
        "split", "--config", &settings, "--seed", "17", "--ticks", "32",
    ]);
    directory.success(&["split"]);
    let full = directory.latest("full");
    let split = directory.latest("split");
    assert_eq!(full.state.step(), 64);
    assert_eq!(full.config.ecosystem, initial.config.ecosystem);
    assert_eq!(full.config.ecosystem, split.config.ecosystem);
    assert_eq!(full.state, split.state);
    for animal in split.state.world().animals().values() {
        assert_eq!(animal.mind.brain.genome.hidden_units, 6);
        assert_eq!(animal.mind.brain.memory.len(), 6);
    }

    // Runtime metric selection may change; canonical model settings may not.
    directory.success(&[
        "split",
        "--ticks",
        "0",
        "--config",
        &directory.overlay("metrics-off.toml", "[ecosystem.metrics]\nenabled = false\n"),
    ]);
    assert_eq!(directory.latest("split").state, full.state);
    let config_path = directory.0.join("split/config.toml");
    let config = fs::read(&config_path).unwrap();
    let checkpoint_path = directory.checkpoints("split").pop().unwrap();
    let checkpoint = fs::read(&checkpoint_path).unwrap();
    let metrics = fs::read(directory.metrics_path("split")).unwrap();
    for replacement in [
        "[ecosystem.world]\nwidth = 81.0\n",
        "[ecosystem.immigration]\nenergy = 71.0\n",
        "[ecosystem.animal.body]\nupkeep = 1.6\n",
        "[ecosystem.animal.body]\nmovement_cost = 0.001\n",
        "[ecosystem.animal.body]\nmovement_energy_power = 0.0\n",
        "[ecosystem.animal.reproduction]\nchild_energy_fraction = 0.4\n",
        "[ecosystem.animal.feeding]\nefficiency = 0.3\n",
        "[ecosystem.plant]\ngrowth = 0.5\n",
        "[ecosystem.sensing]\nsoftening = 0.03\n",
        "[ecosystem.genetics.initialization]\nhidden_units = 7\n",
        "[ecosystem.genetics.mutation]\nmax_delta = 0.05\n",
    ] {
        let replacement = directory.overlay("replacement.toml", replacement);
        let output = directory.run(&["split", "--config", &replacement, "--ticks", "0"]);
        assert!(!output.status.success(), "accepted altered model settings");
        assert_eq!(fs::read(&config_path).unwrap(), config);
        assert_eq!(fs::read(&checkpoint_path).unwrap(), checkpoint);
        assert_eq!(fs::read(directory.metrics_path("split")).unwrap(), metrics);
    }
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
    let Record::Session(session) = &records[0] else {
        panic!("missing metadata")
    };
    assert_eq!(session.start_tick, 0);
    assert!(!session.started_at.is_empty() && !session.code_version.is_empty());
    assert!(!session.world_model.is_empty() && !session.mind_model.is_empty());
    assert_eq!(
        toml::from_str::<toml::Value>(&session.config).unwrap(),
        directory.config("partial")
    );
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
        assert!(v["window"]["active_animal_ticks"].as_u64().unwrap() > 0);
        assert_eq!(
            v.as_object().unwrap().len(),
            9,
            "eight metrics plus window metadata"
        );
        assert!(v["mean_animal_energy"].is_number());
        assert!(v["mean_plant_energy"].is_number());
        assert!(v["distance_per_animal_tick"].is_number());
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
    assert_eq!(
        directory.latest("partial").state,
        directory.latest("disabled").state
    );
}

#[test]
fn live_history_discovers_resume_files_and_preserves_each_invocation() {
    let directory = Directory::new();
    let mut reader = Reader::new(directory.0.join("resumed"));
    assert!(reader.poll().unwrap().is_empty());
    directory.success(&[
        "resumed",
        "-c",
        &support::preset("debug"),
        "-c",
        &directory.overlay("first.toml", "[ecosystem.metrics]\nwindow_ticks = 3\n"),
        "--ticks",
        "2",
    ]);
    let first = drain_metrics(&mut reader);
    assert_eq!(first.len(), 2);
    let path = directory.metrics_path("resumed");
    let bytes = fs::read(&path).unwrap();
    directory.success(&[
        "resumed",
        "-c",
        &directory.overlay("second.toml", "[ecosystem.metrics]\nwindow_ticks = 4\n"),
        "--ticks",
        "5",
    ]);
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(
        fs::read_dir(directory.0.join("resumed/metrics"))
            .unwrap()
            .count(),
        2
    );
    let second = drain_metrics(&mut reader);
    assert_eq!(second.len(), 3);
    for (records, id, start, size) in [(&first, 1, 0, 3), (&second, 2, 2, 4)] {
        let Record::Session(s) = &records[0] else {
            panic!("missing metadata")
        };
        assert_eq!((s.id, s.start_tick), (id, start));
        let config: toml::Value = toml::from_str(&s.config).unwrap();
        assert_eq!(
            config["ecosystem"]["metrics"]["window_ticks"].as_integer(),
            Some(size)
        );
    }
    let points = samples(&second);
    assert_eq!(points.iter().map(|s| s.tick).collect::<Vec<_>>(), [6, 7]);
    assert_eq!(metric_values(points[1])["window"]["samples"], 1);
    assert!(reader.poll().unwrap().is_empty());
    directory.success(&["resumed", "--ticks", "0"]);
    assert_eq!(drain_metrics(&mut reader).len(), 1);
    let restored = directory.latest("resumed").restore();
    let state = restored.universe().read();
    assert!(state.world().metrics_window().is_none());
    assert!(state.metrics().food_per_tick.is_none());
    assert!(state.metrics().mean_animal_energy.is_some());
    drop(state);
    directory.success(&[
        "resumed",
        "-c",
        &directory.overlay("off.toml", "[ecosystem.metrics]\nenabled = false\n"),
        "--ticks",
        "1",
    ]);
    assert!(reader.poll().unwrap().is_empty());
    assert_eq!(
        fs::read_dir(directory.0.join("resumed/metrics"))
            .unwrap()
            .count(),
        3
    );
    directory.success(&["reference", "-c", &support::preset("debug"), "--ticks", "8"]);
    assert_eq!(
        directory.latest("resumed").state,
        directory.latest("reference").state
    );
}

#[test]
fn interrupted_tail_is_preserved_and_later_files_are_followed() {
    let directory = Directory::new();
    directory.success(&["recover", "-c", &support::preset("debug"), "--ticks", "2"]);
    let path = directory.metrics_path("recover");
    let mut reader = Reader::new(directory.0.join("recover"));
    let prior = drain_metrics(&mut reader);
    let record = Record::Sample(Sample {
        session: 1,
        tick: 3,
        values: samples(&prior)[0].values.clone(),
    });
    let mut bytes = Vec::new();
    ciborium::into_writer(&record, &mut bytes).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&bytes[..bytes.len() / 2])
        .unwrap();
    let original = fs::read(&path).unwrap();
    assert!(reader.poll().unwrap().is_empty());
    assert!(reader.has_pending_record());
    directory.success(&["recover", "--ticks", "1"]);
    assert_eq!(fs::read(path).unwrap(), original);
    let next = drain_metrics(&mut reader);
    assert_eq!(next.len(), 2);
    assert_eq!(samples(&next)[0].tick, 3);
    assert_eq!(samples(&next)[0].session, 2);
    assert!(!reader.has_pending_record());
}

#[test]
fn old_corruption_cannot_block_a_new_invocation_or_be_overwritten() {
    let directory = Directory::new();
    directory.success(&["corrupt", "-c", &support::preset("debug"), "--ticks", "2"]);
    let path = directory.metrics_path("corrupt");
    let mut corrupt = fs::read(&path).unwrap();
    corrupt.push(0xff);
    fs::write(&path, &corrupt).unwrap();
    directory.success(&["corrupt", "--ticks", "1"]);
    assert_eq!(fs::read(&path).unwrap(), corrupt);
    assert_eq!(
        fs::read_dir(directory.0.join("corrupt/metrics"))
            .unwrap()
            .count(),
        2
    );
    let mut reader = Reader::new(directory.0.join("corrupt"));
    assert_eq!(reader.poll().unwrap().len(), 2);
    assert!(
        reader.poll().is_err(),
        "viewer must still report old corruption"
    );
}

#[test]
fn resuming_an_older_checkpoint_keeps_overlapping_metric_ticks_in_distinct_sessions() {
    let directory = Directory::new();
    directory.success(&[
        "rollback",
        "--config",
        &support::preset("debug"),
        "--config",
        &support::preset("metrics"),
        "--config",
        &directory.overlay(
            "rollback.toml",
            "checkpoint_every = 2\nkeep_checkpoints = 4\n[ecosystem.metrics]\nwindow_ticks = 1\n",
        ),
        "--ticks",
        "4",
    ]);
    let path = directory.metrics_path("rollback");
    let old = fs::read(&path).unwrap();
    let mut reader = Reader::new(directory.0.join("rollback"));
    let previous = drain_metrics(&mut reader);
    let initial_samples = samples(&previous);
    assert_eq!(
        initial_samples
            .iter()
            .map(|sample| sample.tick)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    fs::remove_file(directory.checkpoints("rollback").last().unwrap()).unwrap();
    directory.success(&["rollback", "--ticks", "2"]);
    assert!(fs::read(&path).unwrap().starts_with(&old));
    let records = drain_metrics(&mut reader);
    let Record::Session(session) = &records[0] else {
        panic!("missing rollback metadata")
    };
    assert_eq!(session.start_tick, 2);
    let appended = samples(&records);
    assert_eq!(
        appended
            .iter()
            .map(|sample| sample.tick)
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert!(appended.iter().all(|sample| sample.session == session.id));
    assert_ne!(session.id, initial_samples[0].session);
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
        directory.latest("zero").restore(),
        Run::new(support::ecosystem("debug"), 7).unwrap()
    );
    directory.success(&["zero", "--ticks", "3"]);
    let mut reference = Run::new(support::ecosystem("debug"), 7).unwrap();
    for _ in 0..3 {
        reference.tick().unwrap();
    }
    assert_eq!(directory.latest("zero").restore(), reference);
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
    assert_eq!(directory.latest("maximum").restore(), reference);
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
    assert_eq!(directory.latest("runs/a-newer").restore(), reference);
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
    assert_eq!(directory.latest("recover").restore(), reference);
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
    let mut metrics_reader = Reader::new(directory.0.join("live"));
    assert!(metrics_reader.poll().unwrap().is_empty());
    let child = directory.spawn(&[
        "live",
        "--seed",
        "1",
        "--tick-hz",
        "50",
        "--config",
        &directory.overlay(
            "policy.toml",
            "checkpoint_every = 1\n[ecosystem.metrics]\nenabled = true\nwindow_ticks = 1\n",
        ),
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
    let mut observed_metrics = Vec::new();
    while samples(&observed_metrics).is_empty() {
        observed_metrics.extend(metrics_reader.poll().unwrap());
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no live metric records arrived"
        );
        if samples(&observed_metrics).is_empty() {
            thread::sleep(Duration::from_millis(5));
        }
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
    observed_metrics.extend(drain_metrics(&mut metrics_reader));
    let points = samples(&observed_metrics);
    assert_eq!(points.last().unwrap().tick, saved.state.step());
    assert_eq!(points.len() as u64, saved.state.step());
    assert!(!metrics_reader.has_pending_record());
    let mut reference = Run::new(support::ecosystem("baseline"), 1).unwrap();
    for _ in 0..saved.state.step() {
        reference.tick().unwrap();
    }
    assert_eq!(saved.restore(), reference);
    // The process released the OS lock; a subsequent invocation can continue.
    directory.success(&["live", "--ticks", "1", "--tick-hz", "0"]);
    let continued = drain_metrics(&mut metrics_reader);
    assert!(matches!(&continued[0], Record::Session(_)));
    assert_eq!(samples(&continued).len(), 1);
    assert_eq!(samples(&continued)[0].tick, points.last().unwrap().tick + 1);
}
