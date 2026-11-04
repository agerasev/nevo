//! Deliberate run controls; nested model and viewer settings belong in config files.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use super::config::Config;

/// Run an experiment. Configure ecosystem and viewer settings in TOML files.
#[derive(Parser, Debug)]
#[command(name = "nevo")]
pub struct Cli {
    /// Run directory: resume if it contains checkpoints, otherwise create a run.
    pub directory: Option<PathBuf>,

    /// Load TOML files in order; later files override earlier ones, then CLI wins.
    #[arg(long)]
    pub config: Vec<PathBuf>,

    /// Override the master seed; new runs draw one from entropy when unspecified.
    #[arg(long)]
    seed: Option<u64>,

    /// Stop after this many additional ticks, including when resuming.
    #[arg(long)]
    ticks: Option<u64>,

    /// Limit simulation ticks per second; zero removes pacing. New runs default to unlimited.
    #[arg(long)]
    tick_hz: Option<f64>,

    /// Enable the viewer; an explicit false overrides viewer.enable in config.
    #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    viewer: Option<bool>,

    /// Start paused; an explicit false overrides saved configuration.
    #[arg(long, num_args = 0..=1, default_missing_value = "true", require_equals = true)]
    paused: Option<bool>,
}

impl Cli {
    pub fn load_config(&self, fallback: Option<&Config>) -> Result<Config> {
        let mut config = Config::load(self.directory.as_deref(), &self.config, fallback)?;
        // Only explicit arguments override config values; defaults live in Config.
        if let Some(value) = self.seed {
            config.seed = Some(value);
        }
        if let Some(value) = self.ticks {
            config.ticks = Some(value);
        }
        if let Some(value) = self.tick_hz {
            config.tick_hz = value;
        }
        if let Some(value) = self.viewer {
            config.viewer.enable = value;
        }
        if let Some(value) = self.paused {
            config.paused = value;
        }
        Ok(config)
    }
}
