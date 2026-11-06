//! Continuous count-density recruitment in the mature-plant inset.
//!
//! Proposals use the cheaper of a uniform envelope and the sum of Gaussian patch
//! bumps plus the uniform floor. Acceptance is `(capacity - density)+ / envelope`.
//! Expected attempts are recovery_rate times envelope mass. Full Gaussian draws
//! outside the arena consume an attempt; there are no internal rejection loops.
//! Density is a Gaussian kernel sum, divided by the kernel mass inside the arena
//! at the query point. This corrects the apparent lack of neighbours at boundaries.
//! Capacity is soft: adding a discrete plant can overshoot it in nearby locations.

use std::f32::consts::{SQRT_2, TAU};

use anyhow::{Result, ensure};
use glam::Vec2;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};

use super::{PatchConfig, Plant, WorldConfig, patches};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Plants per square world unit far from patch centres.
    pub density_floor: f32,
    /// Plants per square world unit at patch centres.
    pub density_peak: f32,
    /// Standard deviation of each plant's normalized Gaussian crowding kernel.
    pub crowding_radius: f32,
    /// Per-tick recovery coefficient: expected proposals = rate * envelope mass.
    pub recovery_rate: f32,
    /// Hard ceiling on total tick proposals, including rejections.
    pub max_attempts_per_tick: usize,
    /// Total proposal budget for establishing the requested initial plant count.
    pub max_initial_attempts: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            density_floor: 0.0006,
            density_peak: 0.0006,
            crowding_radius: 40.0,
            recovery_rate: 0.01,
            max_attempts_per_tick: 128,
            max_initial_attempts: 100_000,
        }
    }
}

impl Config {
    pub(super) fn validate(
        self,
        world: WorldConfig,
        max_radius: f32,
        patches: PatchConfig,
    ) -> Result<()> {
        ensure!(
            self.density_floor.is_finite()
                && self.density_peak.is_finite()
                && self.density_floor >= 0.0
                && self.density_peak > 0.0
                && self.density_floor <= self.density_peak,
            "replenishment densities must be finite, nonnegative, ordered, and have a positive peak"
        );
        ensure!(
            patches.count > 0 || self.density_floor == self.density_peak,
            "varying replenishment capacity requires patch centres"
        );
        ensure!(
            self.crowding_radius.is_finite() && self.crowding_radius > 0.0,
            "crowding_radius must be finite and positive"
        );
        ensure!(
            self.recovery_rate.is_finite() && self.recovery_rate >= 0.0,
            "recovery_rate must be finite and nonnegative"
        );
        let sampler = Sampler::new(self, world, max_radius, patches)?;
        for position in [Vec2::ZERO, sampler.limit] {
            let scale = sampler.kernel_scale(position);
            ensure!(
                scale.is_finite() && scale > 0.0,
                "crowding_radius produces an unrepresentable density kernel"
            );
        }
        let attempts = sampler.expected_attempts();
        ensure!(
            attempts.is_finite()
                && attempts.ceil() < usize::MAX as f32
                && attempts.ceil() as usize <= self.max_attempts_per_tick,
            "recovery_rate requires too many proposals; increase max_attempts_per_tick or lower recovery_rate"
        );
        Ok(())
    }
}

pub(super) struct Sampler {
    config: Config,
    patches: PatchConfig,
    limit: Vec2,
    proposal_mass: f32,
    uniform_mass: f32,
    gaussian_envelope: bool,
}

impl Sampler {
    pub(super) fn new(
        config: Config,
        world: WorldConfig,
        max_radius: f32,
        patches: PatchConfig,
    ) -> Result<Self> {
        let limit = patches::inset(world, max_radius)?;
        let area = (2.0 * limit.x) * (2.0 * limit.y);
        ensure!(
            limit.cmpgt(Vec2::ZERO).all() && area.is_finite() && area > 0.0,
            "replenishment requires a finite positive area for mature plants"
        );
        let uniform_mass = area * config.density_floor;
        let flat_mass = area * config.density_peak;
        let delta = config.density_peak - config.density_floor;
        // Fall back to uniform when Gaussian mass or height is unrepresentable.
        let gaussian_mass =
            uniform_mass + ((delta * patches.spread) * patches.spread) * TAU * patches.count as f32;
        let gaussian_envelope = delta > 0.0
            && patches.count > 0
            && gaussian_mass.is_finite()
            && gaussian_mass > uniform_mass
            && gaussian_mass < flat_mass
            && (config.density_floor + delta * patches.count as f32).is_finite();
        let proposal_mass = if gaussian_envelope {
            gaussian_mass
        } else {
            flat_mass
        };
        ensure!(
            proposal_mass.is_finite() && proposal_mass > 0.0,
            "replenishment requires a finite positive proposal envelope"
        );
        Ok(Self {
            config,
            patches,
            limit,
            proposal_mass,
            uniform_mass,
            gaussian_envelope,
        })
    }

    fn expected_attempts(&self) -> f32 {
        self.config.recovery_rate * self.proposal_mass
    }

    /// Stochastic rounding preserves fractional rates without checkpointed debt.
    pub(super) fn attempts<R: Rng + ?Sized>(&self, rng: &mut R) -> usize {
        let expected = self.expected_attempts();
        let whole = expected.floor() as usize;
        whole + usize::from(expected.fract() > 0.0 && rng.random::<f32>() < expected.fract())
    }

    pub(super) fn propose<R: Rng + ?Sized>(
        &self,
        plants: &[Plant],
        landscape: &patches::Landscape,
        rng: &mut R,
    ) -> Option<Vec2> {
        let position = if self.gaussian_envelope
            && rng.random::<f32>() >= self.uniform_mass / self.proposal_mass
        {
            let center = landscape.centers()[rng.random_range(0..landscape.centers().len())];
            // Box-Muller supplies both axes with two draws and no retry loop.
            let radius = (-2.0 * (1.0 - rng.random::<f32>()).ln()).sqrt();
            let (sin, cos) = (TAU * rng.random::<f32>()).sin_cos();
            center + (self.patches.spread * radius) * Vec2::new(cos, sin)
        } else {
            patches::uniform_position(rng, self.limit)
        };
        if !position.is_finite() || !position.abs().cmple(self.limit).all() {
            return None;
        }
        let probability = self.acceptance(position, plants, landscape);
        (rng.random::<f32>() < probability).then_some(position)
    }

    fn acceptance(&self, position: Vec2, plants: &[Plant], landscape: &patches::Landscape) -> f32 {
        let config = self.config;
        let capacity = config.density_floor
            + (config.density_peak - config.density_floor)
                * landscape.fertility(position, self.patches);
        let scale = self.kernel_scale(position);
        let mut density = 0.0;
        for plant in plants {
            let relative = (position - plant.position) / config.crowding_radius;
            density += scale * (-0.5 * relative.length_squared()).exp();
            if density >= capacity {
                return 0.0;
            }
        }
        let envelope = self.envelope(position, landscape);
        if envelope == 0.0 {
            return 0.0;
        }
        ((capacity - density) / envelope).clamp(0.0, 1.0)
    }

    fn envelope(&self, position: Vec2, landscape: &patches::Landscape) -> f32 {
        if !self.gaussian_envelope {
            return self.config.density_peak;
        }
        let sum: f32 = landscape
            .centers()
            .iter()
            .map(|center| {
                let relative = (position - center) / self.patches.spread;
                (-0.5 * relative.length_squared()).exp()
            })
            .sum();
        self.config.density_floor + (self.config.density_peak - self.config.density_floor) * sum
    }

    fn kernel_scale(&self, position: Vec2) -> f32 {
        let radius = self.config.crowding_radius;
        let retained_mass = |p: f32, limit: f32| {
            0.5 * (libm::erff(((limit - p) / radius) / SQRT_2)
                + libm::erff(((limit + p) / radius) / SQRT_2))
        };
        let normalizer = (1.0 / radius) / TAU.sqrt();
        (normalizer / retained_mass(position.x, self.limit.x))
            * (normalizer / retained_mass(position.y, self.limit.y))
    }
}

#[cfg(test)]
mod tests;
