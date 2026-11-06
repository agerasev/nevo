//! Fixed Gaussian fertility shared by resource capacity and plant growth.

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};

use super::WorldConfig;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Number of fixed centres; zero gives a uniformly zero fertility field.
    pub count: usize,
    /// Standard deviation in world units, shared by both spatial axes.
    pub spread: f64,
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
            self.count > 0 || self.growth_floor == self.growth_peak,
            "varying growth requires at least one patch centre"
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
    pub(super) fn centers(&self) -> &[DVec2] {
        &self.centers
    }

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

    /// Smooth blend in [0, 1]; overlapping Gaussian bumps never exceed one.
    pub(super) fn fertility(&self, position: DVec2, config: Config) -> f64 {
        let unfertile = self.centers.iter().fold(1.0, |remaining, center| {
            let relative = (position - center) / config.spread;
            remaining * (1.0 - (-0.5 * relative.length_squared()).exp())
        });
        1.0 - unfertile
    }

    pub(super) fn growth_multiplier(&self, position: DVec2, config: Config) -> f64 {
        if config.growth_floor == config.growth_peak {
            return config.growth_floor;
        }
        config.growth_floor
            + (config.growth_peak - config.growth_floor) * self.fertility(position, config)
    }
}

pub(super) fn inset(world: WorldConfig, max_radius: f64) -> Result<DVec2> {
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

pub(super) fn uniform_position<R: Rng + ?Sized>(rng: &mut R, limit: DVec2) -> DVec2 {
    DVec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0)) * limit
}

#[cfg(test)]
mod tests;
