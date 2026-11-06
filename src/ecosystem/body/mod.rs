//! Animal disk state and body-relative coordinate helpers.

use std::f64::consts::{PI, TAU};

use anyhow::{Result, ensure};
use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::config::{BodyConfig, BodyParameters, WorldConfig};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MotorAction {
    /// Forward effort, clamped to [0, 1].
    pub thrust: f64,
    /// Signed turning effort, clamped to [-1, 1].
    pub turn: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub position: DVec2,
    pub energy: f64,
    /// Counterclockwise angle from the world x axis, in [-pi, pi).
    pub heading: f64,
    pub velocity: DVec2,
    pub angular_velocity: f64,
    radius_scale: f64,
}

/// Integrated physical quantities; converting these to reserve energy is policy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Motion {
    /// Integrated path length during physical motion.
    pub distance: f64,
    pub mechanical_work: f64,
    pub force_squared_time: f64,
    pub torque_squared_time: f64,
}

impl Body {
    pub fn new(position: DVec2, energy: f64) -> Self {
        Self::with_radius_scale(position, energy, BodyConfig::default().radius_scale)
    }

    pub(super) fn with_radius_scale(position: DVec2, energy: f64, radius_scale: f64) -> Self {
        Self {
            position,
            energy,
            heading: 0.0,
            velocity: DVec2::ZERO,
            angular_velocity: 0.0,
            radius_scale,
        }
    }

    pub fn radius(&self) -> f64 {
        self.radius_scale * self.energy.max(0.0).sqrt()
    }

    pub fn mass(&self, mass_per_energy: f64) -> f64 {
        self.energy.max(0.0) * mass_per_energy
    }

    pub fn inertia(&self, mass_per_energy: f64) -> f64 {
        0.5 * self.mass(mass_per_energy) * self.radius().powi(2)
    }

    pub fn forward(&self) -> DVec2 {
        direction(self.heading)
    }

    /// Express a world-frame vector as forward and left components.
    pub fn world_to_body(&self, vector: DVec2) -> DVec2 {
        let forward = self.forward();
        DVec2::new(forward.dot(vector), forward.perp().dot(vector))
    }

    pub(super) fn validate(&self, world: WorldConfig, parameters: BodyParameters) -> Result<()> {
        // Negative reserve energy is legitimate between upkeep and the next cull.
        ensure!(self.energy.is_finite(), "non-finite animal energy");
        ensure!(
            self.position.is_finite() && self.position.abs().cmple(world.half_size()).all(),
            "invalid animal position"
        );
        ensure!(
            self.radius_scale == parameters.radius_scale && self.radius().is_finite(),
            "invalid animal radius"
        );
        ensure!(
            self.heading.is_finite() && (-PI..PI).contains(&self.heading),
            "invalid animal heading"
        );
        ensure!(
            self.velocity.is_finite() && self.angular_velocity.is_finite(),
            "non-finite animal velocity"
        );
        Ok(())
    }

    /// Clamp to the arena and dissipate outward normal velocity at its walls.
    pub(super) fn confine(&mut self, world: WorldConfig) {
        let limit = (world.half_size() - self.radius()).max(DVec2::ZERO);
        self.position = self.position.clamp(-limit, limit);
        for axis in 0..2 {
            if (self.position[axis] >= limit[axis] && self.velocity[axis] > 0.0)
                || (self.position[axis] <= -limit[axis] && self.velocity[axis] < 0.0)
            {
                self.velocity[axis] = 0.0;
            }
        }
    }
}

fn direction(heading: f64) -> DVec2 {
    let (sin, cos) = heading.sin_cos();
    DVec2::new(cos, sin)
}

pub(super) fn normalize_heading(heading: f64) -> f64 {
    let wrapped = heading.rem_euclid(TAU);
    if wrapped >= PI {
        wrapped - TAU
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests;
