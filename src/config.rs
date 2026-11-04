use std::{env, fmt, fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use conf::Conf;
use nevo::{foraging::Scenario, run::LoopOptions};
use rand::{TryRng, rngs::SysRng};
use serde::de::{self, Deserializer, Visitor};

/// Run a seeded experiment, optionally attaching a viewer.
#[derive(Conf, Debug)]
#[conf(serde, name = "nevo")]
pub struct Options {
    /// Read a TOML configuration file. CLI values override the file.
    #[conf(long = "config", serde(skip))]
    config: Option<PathBuf>,

    /// Initial layout: baseline or debug.
    #[conf(long, default_value = "baseline", serde(use_value_parser))]
    pub scenario: Scenario,

    /// Master seed. If omitted, obtain and print a seed from system entropy.
    #[conf(long, serde(deserialize_with = "deserialize_seed"))]
    pub seed: Option<u64>,

    /// Stop after this many simulation ticks. Omit for an unlimited run.
    #[conf(long)]
    pub ticks: Option<u64>,

    /// Maximum tick rate; zero runs as fast as possible.
    #[conf(long, default_value = "0")]
    pub hz: f64,

    /// Attach the viewer. An explicit false overrides the config file.
    #[conf(parameter, long, default_value = "false", default_if_missing = "true")]
    pub viewer: bool,

    /// Start paused at tick zero.
    #[conf(parameter, long, default_value = "false", default_if_missing = "true")]
    pub paused: bool,

    /// Check twelve displayed snapshots and controls; requires the viewer.
    #[conf(parameter, long, default_value = "false", default_if_missing = "true")]
    pub smoke: bool,
}

impl Options {
    pub fn load() -> Result<Self> {
        let args: Vec<_> = env::args_os().collect();
        let builder = || Self::conf_builder().args(args.clone());
        // Parse once to locate the file and handle help/errors before any file I/O.
        let cli = builder().try_parse()?;
        let Some(path) = &cli.config else {
            return Ok(cli);
        };
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading configuration {}", path.display()))?;
        let document = toml::de::Deserializer::parse(&text)
            .with_context(|| format!("parsing configuration {}", path.display()))?;
        Ok(builder()
            .doc(path.display().to_string(), document)
            .try_parse()?)
    }

    pub fn loop_options(&self) -> Result<LoopOptions> {
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
