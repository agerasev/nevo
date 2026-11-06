use super::*;
use crate::app::cli::Cli;
use clap::Parser;
use std::{
    env,
    sync::atomic::{AtomicU64, Ordering},
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
fn only_explicit_shortcuts_override_saved_settings() {
    let directory = Directory::new();
    fs::write(directory.0.join("config.toml"), "seed = '18446744073709551615'\nticks = 32\npaused = true\n[viewer]\nenable = true\nvsync = false\n").unwrap();
    let config = load(
        &[
            directory.0.to_str().unwrap(),
            "--ticks",
            "5",
            "--viewer=false",
            "--paused=false",
        ],
        None,
    )
    .unwrap();
    assert_eq!(config.seed, Some(u64::MAX));
    assert_eq!(config.ticks, Some(5));
    assert!(!config.paused && !config.viewer.enable && !config.viewer.vsync);
    assert_eq!(
        toml::from_str::<Config>(&config.to_toml().unwrap())
            .unwrap()
            .seed,
        Some(u64::MAX)
    );
}

#[test]
fn precedence_is_checkpoint_saved_file_ordered_overlays_then_cli() {
    let directory = Directory::new();
    let path = directory.0.to_str().unwrap();
    let checkpoint = Config {
        ticks: Some(1),
        ..Config::default()
    };
    assert_eq!(load(&[path], Some(&checkpoint)).unwrap().ticks, Some(1));
    fs::write(directory.0.join("config.toml"), "ticks = 2").unwrap();
    assert_eq!(load(&[path], Some(&checkpoint)).unwrap().ticks, Some(2));
    let first = directory.0.join("first.toml");
    let second = directory.0.join("second.toml");
    fs::write(&first, "ticks = 3").unwrap();
    fs::write(&second, "ticks = 4").unwrap();
    let args = [
        path,
        "-c",
        first.to_str().unwrap(),
        "-c",
        second.to_str().unwrap(),
    ];
    assert_eq!(load(&args, Some(&checkpoint)).unwrap().ticks, Some(4));
    assert_eq!(
        load(&[&args[..], &["--ticks", "5"]].concat(), Some(&checkpoint))
            .unwrap()
            .ticks,
        Some(5)
    );
}

#[test]
fn merging_preserves_sibling_fields_and_replaces_arrays() {
    let mut base = toml::from_str(
        "[viewer]\nenable = true\nvsync = false\n[ecosystem.immigration]\nposition = [1, 2]",
    )
    .unwrap();
    merge(
        &mut base,
        toml::from_str("[viewer]\nenable = false\n[ecosystem.immigration]\nposition = [3, 4]")
            .unwrap(),
    );
    let config: Config = base.try_into().unwrap();
    assert!(!config.viewer.enable && !config.viewer.vsync);
    assert_eq!(
        config.ecosystem.immigration.position,
        nevo::ecosystem::Placement::Fixed(glam::Vec2::new(3.0, 4.0))
    );
}

#[test]
fn overwritten_files_still_reject_unknown_keys_and_invalid_types() {
    let directory = Directory::new();
    let first = directory.0.join("first.toml");
    let second = directory.0.join("second.toml");
    fs::write(&second, "ticks = 0").unwrap();
    for content in ["ticks = 'wrong'", "[viewer]\nunknown = true"] {
        fs::write(&first, content).unwrap();
        assert!(Config::load(None, &[first.clone(), second.clone()], None).is_err());
    }
}

#[test]
fn movement_presets_each_specify_a_complete_energy_budget() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs/movement");
    for name in ["upkeep-only", "work-and-loss"] {
        let text = fs::read_to_string(root.join(format!("{name}.toml"))).unwrap();
        let value: toml::Value = toml::from_str(&text).unwrap();
        for field in [
            "upkeep",
            "energy_scale",
            "motor_efficiency",
            "force_loss",
            "torque_loss",
        ] {
            assert!(
                value["ecosystem"]["animal"]["body"].get(field).is_some(),
                "{name}: {field}"
            );
        }
    }
}

#[test]
fn preset_categories_own_disjoint_fields_and_compose_independently_of_order() {
    use std::collections::BTreeMap;

    fn claim_leaves<'a>(
        value: &toml::Value,
        path: String,
        category: &'a str,
        owners: &mut BTreeMap<String, &'a str>,
    ) {
        if let toml::Value::Table(table) = value {
            for (key, value) in table {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                claim_leaves(value, path, category, owners);
            }
        } else if let Some(previous) = owners.insert(path.clone(), category) {
            assert_eq!(previous, category, "preset categories both own {path}");
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("configs");
    let categories: &[(&str, &[&str])] = &[
        (
            "world",
            &[
                "small-uniform",
                "large-uniform",
                "small-nonuniform",
                "large-nonuniform",
                "debug",
            ],
        ),
        ("locomotion", &["damped"]),
        ("obstacles", &["none", "scattered", "scattered-small"]),
        ("movement", &["upkeep-only", "work-and-loss"]),
        ("plant-growth", &["uniform", "patches"]),
        ("viewer", &["window"]),
        ("metrics", &["long-window"]),
        ("diagnostics", &["trace"]),
    ];
    let mut owners = BTreeMap::new();
    for &(category, names) in categories {
        for name in names {
            let path = root.join(category).join(format!("{name}.toml"));
            let value: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert!(
                value.get("ticks").is_none(),
                "{} sets a run duration",
                path.display()
            );
            claim_leaves(&value, String::new(), category, &mut owners);
            if category == "world" {
                let config = Config::load(None, &[path], None).unwrap();
                config.ecosystem.validate().unwrap();
                assert_eq!(config.ticks, None);
            }
        }
    }

    let mut paths = [
        "world/large-nonuniform.toml",
        "locomotion/damped.toml",
        "obstacles/scattered.toml",
        "movement/work-and-loss.toml",
        "plant-growth/patches.toml",
        "viewer/window.toml",
        "metrics/long-window.toml",
        "diagnostics/trace.toml",
    ]
    .map(|path| root.join(path));
    let forward = Config::load(None, &paths, None).unwrap();
    forward.ecosystem.validate().unwrap();
    assert_eq!(forward.ticks, None);
    paths.reverse();
    assert_eq!(Config::load(None, &paths, None).unwrap(), forward);
}

#[test]
fn enabling_the_viewer_does_not_enable_simulation_pacing() {
    assert_eq!(load(&["--viewer"], None).unwrap().tick_hz, 0.0);
}

#[test]
fn obsolete_settings_are_accepted_only_in_saved_configuration() {
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
        old.pointer_mut(parent).unwrap()["metrics"] = serde_json::json!({"energy": {"min": true}});
    }
    let path = directory.0.join("config.toml");
    fs::write(&path, toml::to_string(&old).unwrap()).unwrap();
    assert_eq!(
        load(&[directory.0.to_str().unwrap()], None).unwrap(),
        Config::default()
    );
    assert!(load(&["--config", path.to_str().unwrap()], None).is_err());
}
