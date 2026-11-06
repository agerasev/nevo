//! Fixed Gaussian fertility and bounded plant placement within the arena.
//!
//! Clustered placement gives each truncated Gaussian equal weight. Growth uses a
//! bounded blend of their bumps, rather than the normalized placement density.

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{Rng, RngExt};
use rand_distr::{Distribution, StandardNormal};
use serde::{Deserialize, Serialize};

use super::{WorldConfig, random_position};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Number of fixed centres; zero retains uniform placement.
    pub count: usize,
    /// Standard deviation in world units, shared by both spatial axes.
    pub spread: f64,
    /// Fraction of random plant placements drawn around a patch centre.
    pub density_fraction: f64,
    /// Multiplier on plant growth far from all patch centres.
    pub growth_floor: f64,
    /// Multiplier at a patch centre; overlapping patches do not exceed it.
    pub growth_peak: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            count: 0,
            spread: 100.0,
            density_fraction: 0.0,
            growth_floor: 1.0,
            growth_peak: 1.0,
        }
    }
}

impl Config {
    pub fn validate(self, world: WorldConfig, max_radius: f64, base_growth: f64) -> Result<()> {
        ensure!(
            self.spread.is_finite() && self.spread > 0.0,
            "patch spread must be finite and positive"
        );
        ensure!(
            self.density_fraction.is_finite() && (0.0..=1.0).contains(&self.density_fraction),
            "patch density_fraction must be between zero and one"
        );
        ensure!(
            self.growth_floor.is_finite()
                && self.growth_peak.is_finite()
                && self.growth_floor >= 0.0
                && self.growth_peak >= self.growth_floor,
            "patch growth multipliers must be finite, nonnegative, and ordered"
        );
        ensure!(
            base_growth.is_finite()
                && base_growth >= 0.0
                && (base_growth * self.growth_peak).is_finite(),
            "maximum patch growth must be finite and nonnegative"
        );
        ensure!(
            self.count <= isize::MAX as usize / size_of::<DVec2>(),
            "patch count produces an oversized landscape"
        );
        ensure!(
            self.count > 0
                || (self.density_fraction == 0.0 && self.growth_floor == self.growth_peak),
            "patch density and varying growth require at least one centre"
        );
        let limit = inset(world, max_radius)?;
        ensure!(
            self.count == 0 || limit.cmpgt(DVec2::ZERO).all(),
            "patches require room for mature plants inside the arena"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Landscape {
    centers: Vec<DVec2>,
}

impl Landscape {
    pub(super) fn generate<R: Rng + ?Sized>(
        config: Config,
        world: WorldConfig,
        max_radius: f64,
        rng: &mut R,
    ) -> Result<Self> {
        config.validate(world, max_radius, 0.0)?;
        let limit = inset(world, max_radius)?;
        let mut centers = Vec::new();
        centers.try_reserve_exact(config.count)?;
        centers.extend((0..config.count).map(|_| uniform_position(rng, limit)));
        Ok(Self { centers })
    }

    pub(super) fn validate(
        &self,
        config: Config,
        world: WorldConfig,
        max_radius: f64,
    ) -> Result<()> {
        config.validate(world, max_radius, 0.0)?;
        ensure!(
            self.centers.len() == config.count,
            "stored patch count differs from the configuration"
        );
        let limit = inset(world, max_radius)?;
        ensure!(
            self.centers
                .iter()
                .all(|p| p.is_finite() && p.abs().cmple(limit).all()),
            "patch centres must be finite and inside the mature-plant inset"
        );
        Ok(())
    }

    pub(super) fn position<R: Rng + ?Sized>(
        &self,
        config: Config,
        world: WorldConfig,
        max_radius: f64,
        rng: &mut R,
    ) -> Result<DVec2> {
        if config.count == 0 {
            return Ok(random_position(rng, world));
        }
        let limit = inset(world, max_radius)?;
        if config.density_fraction == 0.0
            || (config.density_fraction < 1.0 && rng.random::<f64>() >= config.density_fraction)
        {
            return Ok(uniform_position(rng, limit));
        }
        ensure!(!self.centers.is_empty(), "patch landscape has no centres");
        // Keep the chosen centre through rejection: edge patches retain equal weight.
        let center = self.centers[rng.random_range(0..self.centers.len())];
        Ok(DVec2::new(
            sample_axis(center.x, config.spread, limit.x, rng)?,
            sample_axis(center.y, config.spread, limit.y, rng)?,
        ))
    }

    pub(super) fn growth_multiplier(&self, position: DVec2, config: Config) -> f64 {
        if config.growth_floor == config.growth_peak {
            return config.growth_floor;
        }
        let unfertile = self.centers.iter().fold(1.0, |remaining, center| {
            let relative = (position - center) / config.spread;
            remaining * (1.0 - (-0.5 * relative.length_squared()).exp())
        });
        config.growth_floor + (config.growth_peak - config.growth_floor) * (1.0 - unfertile)
    }
}

fn inset(world: WorldConfig, max_radius: f64) -> Result<DVec2> {
    let half_size = world.half_size();
    ensure!(
        half_size.is_finite() && half_size.cmpgt(DVec2::ZERO).all(),
        "patch arena dimensions must be finite and positive"
    );
    ensure!(
        max_radius.is_finite() && max_radius >= 0.0,
        "maximum plant radius must be finite and nonnegative"
    );
    Ok(half_size - max_radius)
}

fn uniform_position<R: Rng + ?Sized>(rng: &mut R, limit: DVec2) -> DVec2 {
    DVec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * limit
}

fn sample_axis<R: Rng + ?Sized>(center: f64, spread: f64, limit: f64, rng: &mut R) -> Result<f64> {
    for _ in 0..256 {
        if spread <= 2.0 * limit {
            let normal: f64 = StandardNormal.sample(rng);
            let position = center + spread * normal;
            if (-limit..=limit).contains(&position) {
                return Ok(position);
            }
        } else {
            let position = rng.random_range(-1.0..1.0) * limit;
            // Dividing first also avoids overflow for very large arenas.
            let relative = position / spread - center / spread;
            if rng.random::<f64>() < (-0.5 * relative * relative).exp() {
                return Ok(position);
            }
        }
    }
    anyhow::bail!("could not sample a plant position within the arena")
}

#[cfg(test)]
mod tests;
