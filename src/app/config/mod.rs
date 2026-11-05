//! Persisted configuration, independent of the command-line interface.

mod seed;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use nevo::{
    ecosystem,
    run::{LoopOptions, tick_interval},
};
use rand::{TryRng, rngs::SysRng};
use serde::{Deserialize, Serialize};

use seed::{deserialize_seed, serialize_seed};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerTab {
    #[default]
    World,
    Metrics,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewerConfig {
    pub enable: bool,
    pub vsync: bool,
    pub tab: ViewerTab,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            enable: false,
            vsync: true,
            tab: ViewerTab::World,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub ecosystem: ecosystem::Config,
    pub trace_ticks: u64,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_seed",
        deserialize_with = "deserialize_seed"
    )]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks: Option<u64>,
    pub checkpoint_every: u64,
    pub keep_checkpoints: usize,
    /// Zero means unlimited simulation throughput; independent of rendering.
    pub tick_hz: f64,
    pub viewer: ViewerConfig,
    pub paused: bool,
    pub smoke: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ecosystem: ecosystem::Config::default(),
            trace_ticks: 0,
            seed: None,
            ticks: None,
            checkpoint_every: 10000,
            keep_checkpoints: 3,
            tick_hz: 0.0,
            viewer: ViewerConfig::default(),
            paused: false,
            smoke: false,
        }
    }
}

impl Config {
    pub fn load(
        directory: Option<&Path>,
        files: &[PathBuf],
        fallback: Option<&Self>,
    ) -> Result<Self> {
        let saved = directory
            .map(|directory| directory.join("config.toml"))
            .filter(|path| path.exists());
        let mut merged = toml::Value::Table(Default::default());
        if saved.is_none()
            && let Some(fallback) = fallback
        {
            merged = toml::from_str(&fallback.to_toml()?)?;
        }
        for path in saved.iter().chain(files) {
            let name = path.display().to_string();
            let text = fs::read_to_string(path)
                .with_context(|| format!("reading configuration {name}"))?;
            // Validate every layer before merging, but merge only explicitly set fields.
            toml::from_str::<Self>(&text)
                .with_context(|| format!("parsing configuration {name}"))?;
            let document = toml::from_str(&text)?;
            merge(&mut merged, document);
        }
        merged.try_into().context("parsing merged configuration")
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
        let interval = tick_interval(self.tick_hz)?;
        ensure!(
            !self.viewer.enable || cfg!(feature = "viewer"),
            "this build does not include the viewer feature"
        );
        ensure!(
            !self.smoke || self.viewer.enable,
            "smoke requires the viewer"
        );
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

// Tables merge recursively; scalar values and arrays replace the previous value.
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

#[cfg(test)]
mod tests;
