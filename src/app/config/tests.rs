use super::*;
use crate::app::cli::Cli;
use clap::Parser;
use std::{
    env,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        loop {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("nevo-saved-config-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create configuration test directory: {error}"),
            }
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn load(args: &[&str], fallback: Option<&Config>) -> Result<Config> {
    Cli::try_parse_from(std::iter::once("nevo").chain(args.iter().copied()))?.load_config(fallback)
}

#[test]
fn saved_configuration_loads_with_only_explicit_cli_overrides() {
    let directory = Directory::new();
    let original: Config = toml::from_str("seed = '18446744073709551615'\nticks = 32\ncheckpoint_every = 7\nkeep_checkpoints = 2\ntick_hz = 25\npaused = true\n[ecosystem]\ninitial_animals = 1\nimmigration.position = [0, 0]\n[viewer]\nenable = true\nvsync = false\ntab = 'metrics'\n").unwrap();
    let saved = original.to_toml().unwrap();
    assert!(saved.contains("seed = \"18446744073709551615\""));
    assert!(!saved.contains("directory"));
    assert!(!saved.contains("config ="));
    fs::write(directory.0.join("config.toml"), saved).unwrap();

    let restored = load(
        &[
            directory.0.to_str().unwrap(),
            "--ticks",
            "5",
            "--paused=false",
            "--viewer=false",
        ],
        None,
    )
    .unwrap();
    assert_eq!(restored.ecosystem, original.ecosystem);
    assert_eq!(restored.seed, Some(u64::MAX));
    assert_eq!(restored.ticks, Some(5));
    assert_eq!(restored.checkpoint_every, 7);
    assert_eq!(restored.keep_checkpoints, 2);
    assert_eq!(restored.tick_hz, 25.0);
    assert!(!restored.paused);
    assert!(!restored.viewer.enable);
    assert_eq!(restored.viewer.tab, ViewerTab::Metrics);
    assert!(
        !restored.viewer.vsync,
        "viewer shortcut must preserve other display settings"
    );

    let mut bytes = Vec::new();
    ciborium::into_writer(&restored, &mut bytes).unwrap();
    let decoded: Config = ciborium::from_reader(&bytes[..]).unwrap();
    assert_eq!(decoded, restored);
}

#[test]
fn checkpoint_recovers_missing_config_but_existing_file_takes_precedence() {
    let directory = Directory::new();
    let path = directory.0.to_str().unwrap();
    let saved: Config = toml::from_str("seed = 42\nticks = 9\ncheckpoint_every = 7\nkeep_checkpoints = 2\n[ecosystem]\ninitial_animals = 1\n").unwrap();
    let overlay = directory.0.join("overlay.toml");
    fs::write(&overlay, "ticks = 3\n[viewer]\nvsync = false\n").unwrap();
    let recovered = load(
        &[path, "--config", overlay.to_str().unwrap(), "--ticks", "5"],
        Some(&saved),
    )
    .unwrap();
    assert_eq!(recovered.ecosystem, saved.ecosystem);
    assert_eq!(recovered.seed, Some(42));
    assert_eq!(recovered.ticks, Some(5));
    assert_eq!(recovered.checkpoint_every, 7);
    assert_eq!(recovered.keep_checkpoints, 2);
    assert!(!recovered.viewer.vsync);
    let error = load(&[path, "--config", "missing.toml"], Some(&saved)).unwrap_err();
    assert!(error.to_string().contains("reading configuration"));

    fs::write(
        directory.0.join("config.toml"),
        "seed = 42\nticks = 4\ncheckpoint_every = 8\n",
    )
    .unwrap();
    let restored = load(&[path, "--ticks", "6"], Some(&saved)).unwrap();
    assert_eq!(restored.ticks, Some(6));
    assert_eq!(restored.checkpoint_every, 8);
}

#[test]
fn each_cost_preset_replaces_the_complete_energy_budget() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs");
    let presets = [
        ("movement/upkeep-only.toml", 0.005, 0.0, 0.0),
        ("movement/flat.toml", 0.0005, 0.00045, 0.0),
        ("movement/cost.toml", 0.0005, 0.003, 1.0),
    ];
    for (first, ..) in presets {
        for (last, upkeep, cost, power) in presets {
            let first = root.join(first);
            let last = root.join(last);
            let config = load(
                &[
                    "--config",
                    first.to_str().unwrap(),
                    "--config",
                    last.to_str().unwrap(),
                ],
                None,
            )
            .unwrap();
            let body = config.ecosystem.animal.body;
            assert_eq!(
                (body.upkeep, body.movement_cost, body.movement_energy_power),
                (upkeep, cost, power)
            );
            assert_eq!(config.ecosystem.animal.reproduction.threshold, 1.0);
            assert_eq!(config.ecosystem.immigration.energy, 0.25);
            assert_eq!(config.ecosystem.plant.max_energy, 1.0);
        }
    }
}

#[test]
fn viewer_preset_and_shortcut_never_enable_simulation_pacing() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/viewer.toml");
    let preset = load(&["--config", path.to_str().unwrap()], None).unwrap();
    assert!(preset.viewer.enable);
    assert!(preset.viewer.vsync);
    assert_eq!(preset.tick_hz, 0.0);
    assert_eq!(preset, load(&["--viewer"], None).unwrap());
    let default = load(&[], None).unwrap();
    assert!(!default.viewer.enable);
    assert!(default.viewer.vsync);
    assert_eq!(default.loop_options().unwrap().interval, Duration::ZERO);
    let paced = load(&["--tick-hz", "25"], None).unwrap();
    assert_eq!(
        paced.loop_options().unwrap().interval,
        Duration::from_millis(40)
    );
    assert!(load(&["--hz", "25"], None).is_err());
    let unpaced = load(&["--tick-hz", "0"], Some(&paced)).unwrap();
    assert_eq!(unpaced.loop_options().unwrap().interval, Duration::ZERO);
}

#[test]
fn obsolete_saved_settings_are_discarded_but_new_overlays_reject_them() {
    let directory = Directory::new();
    let mut old = serde_json::to_value(Config::default()).unwrap();
    old["smoke"] = false.into();
    old["ecosystem"]["metrics"]["genetic_diversity"] = true.into();
    old["ecosystem"]["metrics"]["population"] = true.into();
    old["ecosystem"]["metrics"]["event_rates"] = true.into();
    for parent in [
        "/ecosystem/animal",
        "/ecosystem/animal/body",
        "/ecosystem/animal/mind/brain",
        "/ecosystem/plant",
    ] {
        old.pointer_mut(parent).unwrap()["metrics"] =
            serde_json::json!({"energy": {"min": true}, "saturation": true});
    }
    assert!(serde_json::from_value::<Config>(old.clone()).is_err());
    assert_eq!(
        Config::from_saved_value(old.clone()).unwrap(),
        Config::default()
    );
    let path = directory.0.join("config.toml");
    fs::write(&path, toml::to_string(&old).unwrap()).unwrap();
    assert_eq!(
        load(&[directory.0.to_str().unwrap()], None).unwrap(),
        Config::default()
    );
    assert!(load(&["--config", path.to_str().unwrap()], None).is_err());

    // Checkpoint configuration uses the same saved-data conversion.
    #[derive(Deserialize)]
    struct Stored {
        #[serde(deserialize_with = "Config::deserialize_saved")]
        config: Config,
    }
    let mut bytes = Vec::new();
    ciborium::into_writer(&serde_json::json!({"config": old}), &mut bytes).unwrap();
    let stored: Stored = ciborium::from_reader(&bytes[..]).unwrap();
    assert_eq!(stored.config, Config::default());
}
