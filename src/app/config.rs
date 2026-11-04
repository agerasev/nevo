use std::{env, ffi::OsString, fmt, fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use conf::Conf;
use nevo::{ecosystem, run::LoopOptions};
use rand::{TryRng, rngs::SysRng};
use serde::{
    Deserialize, Serialize, Serializer,
    de::{self, Deserializer, Visitor},
};

/// Run a seeded experiment, optionally attaching a viewer.
#[derive(Conf, Clone, Debug, Serialize, Deserialize)]
#[conf(serde, name = "nevo", test)]
#[serde(deny_unknown_fields)]
pub struct Options {
    /// Run directory: resume if it contains checkpoints, otherwise create a run.
    #[conf(pos, serde(skip))]
    #[serde(skip)]
    pub directory: Option<PathBuf>,

    /// Read TOML files in order; later files override earlier ones, then CLI wins.
    #[conf(repeat, long = "config", serde(skip))]
    #[serde(skip)]
    pub config: Vec<PathBuf>,

    /// Ecosystem initialization and population support.
    #[conf(flatten, long_prefix)]
    pub ecosystem: ecosystem::Config,

    /// Print decisions and events through this absolute tick; zero disables tracing.
    #[conf(long, default_value = "0")]
    pub trace_ticks: u64,

    /// Master seed. If omitted, obtain and print a seed from system entropy.
    #[conf(long, serde(deserialize_with = "deserialize_seed"))]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_seed",
        deserialize_with = "deserialize_seed"
    )]
    pub seed: Option<u64>,

    /// Stop after this many additional ticks, including when resuming a run.
    #[conf(long)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks: Option<u64>,

    /// Save every N world ticks; zero saves only when the run exits.
    #[conf(long, default_value = "10000")]
    pub checkpoint_every: u64,

    /// Number of newest checkpoints to retain. Must be at least one.
    #[conf(long, default_value = "3")]
    pub keep_checkpoints: usize,

    /// Maximum tick rate; zero runs as fast as possible.
    #[conf(long, default_value = "0")]
    pub hz: f64,

    /// Attach the viewer. An explicit false overrides the config file.
    #[conf(parameter, long, default_value = "false", default_if_missing = "true")]
    pub viewer: bool,

    /// Start paused at the current tick.
    #[conf(parameter, long, default_value = "false", default_if_missing = "true")]
    pub paused: bool,

    /// Check twelve displayed snapshots and controls; requires the viewer.
    #[conf(parameter, long, default_value = "false", default_if_missing = "true")]
    pub smoke: bool,
}

impl Options {
    pub fn load() -> Result<Self> {
        Self::load_from(env::args_os().collect(), None)
    }

    /// Recover configuration from a checkpoint when the run's TOML file is absent.
    pub fn load_with_fallback(saved: &Self) -> Result<Self> {
        Self::load_from(env::args_os().collect(), Some(saved))
    }

    fn load_from(args: Vec<OsString>, fallback: Option<&Self>) -> Result<Self> {
        let builder = || Self::conf_builder().args(args.clone());
        // Parse once to locate the file and handle help/errors before any file I/O.
        let cli = builder().try_parse()?;
        let saved_config = cli
            .directory
            .as_ref()
            .map(|directory| directory.join("config.toml"))
            .filter(|path| path.exists());
        let mut merged = toml::Value::Table(Default::default());
        if saved_config.is_none()
            && let Some(saved) = fallback
        {
            merged = toml::from_str(&saved.to_toml()?)?;
        }
        for path in saved_config.iter().chain(&cli.config) {
            let name = path.display().to_string();
            let text = fs::read_to_string(path)
                .with_context(|| format!("reading configuration {name}"))?;
            let document: toml::Value =
                toml::from_str(&text).with_context(|| format!("parsing configuration {name}"))?;
            // Reject bad keys/types in every file, even when later files hide them.
            Self::conf_builder()
                .args(["nevo"])
                .doc(&name, document.clone())
                .try_parse()?;
            merge(&mut merged, document);
        }
        Ok(builder().doc("merged configuration", merged).try_parse()?)
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("serializing run configuration")
    }

    pub fn loop_options(&self) -> Result<LoopOptions> {
        self.ecosystem.validate()?;
        ensure!(
            self.keep_checkpoints > 0,
            "keep-checkpoints must be at least one"
        );
        ensure!(
            self.hz.is_finite() && self.hz >= 0.0,
            "hz must be finite and nonnegative"
        );
        let interval = if self.hz == 0.0 {
            Duration::ZERO
        } else {
            let interval = Duration::try_from_secs_f64(1.0 / self.hz).context("hz is too small")?;
            ensure!(!interval.is_zero(), "hz is too large");
            interval
        };
        ensure!(
            !self.viewer || cfg!(feature = "viewer"),
            "this build does not include the viewer feature"
        );
        ensure!(!self.smoke || self.viewer, "smoke requires the viewer");
        Ok(LoopOptions {
            ticks: self.ticks,
            interval,
        })
    }

    pub fn resolved_seed(&self) -> Result<u64> {
        match self.seed {
            Some(seed) => Ok(seed),
            None => SysRng
                .try_next_u64()
                .context("obtaining a seed from system entropy"),
        }
    }
}

// Merge tables recursively; scalar values and arrays are replaced as a whole.
fn merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base), toml::Value::Table(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(previous) => merge(previous, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

// TOML integers are signed; quoted seeds also cover the entire u64 entropy range.
fn serialize_seed<S: Serializer>(seed: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
    match seed {
        Some(seed) => serializer.serialize_str(&seed.to_string()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_seed<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    struct Seed;
    impl Visitor<'_> for Seed {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a nonnegative integer or a quoted u64 seed")
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<u64, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<u64, E> {
            value.parse().map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Seed).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

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

    fn load(args: &[&str]) -> Result<Options> {
        load_with(args, None)
    }

    fn load_with(args: &[&str], fallback: Option<&Options>) -> Result<Options> {
        Options::load_from(
            std::iter::once("nevo")
                .chain(args.iter().copied())
                .map(OsString::from)
                .collect(),
            fallback,
        )
    }

    #[test]
    fn saved_configuration_loads_from_directory_with_cli_overrides() {
        let directory = Directory::new();
        let original = load(&[
            "--ecosystem-initial-animals",
            "1",
            "--ecosystem-initial-animal-position",
            "0,0",
            "--seed",
            "18446744073709551615",
            "--ticks",
            "32",
            "--checkpoint-every",
            "7",
            "--keep-checkpoints",
            "2",
        ])
        .unwrap();
        let saved = original.to_toml().unwrap();
        assert!(saved.contains("seed = \"18446744073709551615\""));
        assert!(!saved.contains("directory"));
        assert!(!saved.contains("config ="));
        fs::write(directory.0.join("config.toml"), &saved).unwrap();

        let restored = load(&[
            directory.0.to_str().unwrap(),
            "--ticks",
            "5",
            "--checkpoint-every",
            "0",
        ])
        .unwrap();
        assert_eq!(restored.ecosystem, original.ecosystem);
        assert_eq!(restored.seed, Some(u64::MAX));
        assert_eq!(restored.ticks, Some(5));
        assert_eq!(restored.checkpoint_every, 0);
        assert_eq!(restored.keep_checkpoints, 2);
        assert_eq!(restored.directory.as_ref(), Some(&directory.0));

        let mut bytes = Vec::new();
        ciborium::into_writer(&restored, &mut bytes).unwrap();
        let checkpoint_options: Options = ciborium::from_reader(&bytes[..]).unwrap();
        assert_eq!(checkpoint_options.seed, Some(u64::MAX));
        assert!(checkpoint_options.directory.is_none());
        assert_eq!(
            checkpoint_options.to_toml().unwrap(),
            restored.to_toml().unwrap()
        );
    }

    #[test]
    fn existing_run_loads_an_overlay_and_rejects_zero_retention() {
        let directory = Directory::new();
        let path = directory.0.join("config.toml");
        fs::write(&path, "ticks = 0\nseed = 42\n").unwrap();
        let overlay = directory.0.join("overlay.toml");
        fs::write(&overlay, "ticks = 7\n").unwrap();
        let restored = load(&[
            directory.0.to_str().unwrap(),
            "--config",
            overlay.to_str().unwrap(),
        ])
        .unwrap();
        assert_eq!(restored.ticks, Some(7));
        assert_eq!(restored.seed, Some(42));
        let options = load(&["--keep-checkpoints", "0"]).unwrap();
        assert!(options.loop_options().is_err());
    }

    #[test]
    fn checkpoint_recovers_missing_config_but_existing_file_takes_precedence() {
        let directory = Directory::new();
        let path = directory.0.to_str().unwrap();
        let saved = load(&[
            "--ecosystem-initial-animals",
            "1",
            "--ecosystem-initial-animal-position",
            "0,0",
            "--seed",
            "42",
            "--ticks",
            "9",
            "--checkpoint-every",
            "7",
            "--keep-checkpoints",
            "2",
        ])
        .unwrap();
        let recovered = load_with(&[path, "--ticks", "5"], Some(&saved)).unwrap();
        assert_eq!(recovered.ecosystem, saved.ecosystem);
        assert_eq!(recovered.seed, Some(42));
        assert_eq!(recovered.ticks, Some(5));
        assert_eq!(recovered.checkpoint_every, 7);
        assert_eq!(recovered.keep_checkpoints, 2);
        assert_eq!(recovered.directory.as_ref(), Some(&directory.0));

        let error = load_with(&[path, "--config", "missing.toml"], Some(&saved)).unwrap_err();
        assert!(error.to_string().contains("reading configuration"));

        fs::write(
            directory.0.join("config.toml"),
            "seed = 42\nticks = 4\ncheckpoint_every = 8\n",
        )
        .unwrap();
        let restored = load_with(&[path, "--ticks", "6"], Some(&saved)).unwrap();
        assert_eq!(restored.ticks, Some(6));
        assert_eq!(restored.checkpoint_every, 8);
    }
}
