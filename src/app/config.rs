use std::{env, ffi::OsString, fmt, fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use conf::Conf;
use nevo::{environment::ecosystem::Scenario, run::LoopOptions};
use rand::{TryRng, rngs::SysRng};
use serde::{
    Deserialize, Serialize, Serializer,
    de::{self, Deserializer, Visitor},
};

/// Run a seeded experiment, optionally attaching a viewer.
#[derive(Conf, Clone, Debug, Serialize, Deserialize)]
#[conf(serde, name = "nevo")]
pub struct Options {
    /// Run directory: resume if it contains checkpoints, otherwise create a run.
    #[conf(pos, serde(skip))]
    #[serde(skip)]
    pub directory: Option<PathBuf>,

    /// Read a TOML configuration file. CLI values override the file.
    #[conf(long = "config", serde(skip))]
    #[serde(skip)]
    pub config: Option<PathBuf>,

    /// Initial layout: baseline or debug.
    #[conf(long, default_value = "baseline", serde(use_value_parser))]
    pub scenario: Scenario,

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
        ensure!(
            cli.config.is_none() || (saved_config.is_none() && fallback.is_none()),
            "--config cannot replace an existing run's configuration; use CLI overrides"
        );
        let (name, text) = match cli.config.as_ref().or(saved_config.as_ref()) {
            Some(path) => (
                path.display().to_string(),
                fs::read_to_string(path)
                    .with_context(|| format!("reading configuration {}", path.display()))?,
            ),
            None => match fallback {
                Some(saved) => ("checkpoint configuration".to_string(), saved.to_toml()?),
                None => return Ok(cli),
            },
        };
        let document = toml::de::Deserializer::parse(&text)
            .with_context(|| format!("parsing configuration {name}"))?;
        Ok(builder().doc(name, document).try_parse()?)
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("serializing run configuration")
    }

    pub fn loop_options(&self) -> Result<LoopOptions> {
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
            "--scenario",
            "debug",
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
        assert_eq!(restored.scenario, Scenario::Debug);
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
    fn existing_run_rejects_another_config_and_zero_retention() {
        let directory = Directory::new();
        let path = directory.0.join("config.toml");
        fs::write(&path, "ticks = 0\n").unwrap();
        let error = load(&[
            directory.0.to_str().unwrap(),
            "--config",
            path.to_str().unwrap(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("--config cannot replace"));
        let options = load(&["--keep-checkpoints", "0"]).unwrap();
        assert!(options.loop_options().is_err());
    }

    #[test]
    fn checkpoint_recovers_missing_config_but_existing_file_takes_precedence() {
        let directory = Directory::new();
        let path = directory.0.to_str().unwrap();
        let saved = load(&[
            "--scenario",
            "debug",
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
        assert_eq!(recovered.scenario, Scenario::Debug);
        assert_eq!(recovered.seed, Some(42));
        assert_eq!(recovered.ticks, Some(5));
        assert_eq!(recovered.checkpoint_every, 7);
        assert_eq!(recovered.keep_checkpoints, 2);
        assert_eq!(recovered.directory.as_ref(), Some(&directory.0));

        let error = load_with(&[path, "--config", "missing.toml"], Some(&saved)).unwrap_err();
        assert!(error.to_string().contains("--config cannot replace"));

        fs::write(
            directory.0.join("config.toml"),
            "scenario = 'debug'\nseed = 42\nticks = 4\ncheckpoint_every = 8\n",
        )
        .unwrap();
        let restored = load_with(&[path, "--ticks", "6"], Some(&saved)).unwrap();
        assert_eq!(restored.ticks, Some(6));
        assert_eq!(restored.checkpoint_every, 8);
    }
}
