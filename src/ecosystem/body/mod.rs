//! Damped body motion, independent of reserve-energy accounting and ecology.

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
    /// Sum of substep displacements after wall confinement.
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

    pub(super) fn advance(
        &mut self,
        action: MotorAction,
        parameters: BodyParameters,
        world: WorldConfig,
    ) -> Result<Motion> {
        ensure!(
            action.thrust.is_finite() && action.turn.is_finite(),
            "non-finite motor action"
        );
        self.validate(world, parameters)?;
        let acceleration = action.thrust.clamp(0.0, 1.0) * parameters.max_acceleration;
        let angular_acceleration =
            action.turn.clamp(-1.0, 1.0) * parameters.max_angular_acceleration;
        let force = parameters.mass * acceleration;
        let torque = parameters.moment_of_inertia * angular_acceleration;
        let step = parameters.time_step / f64::from(parameters.substeps);
        let linear = Integration::new(parameters.linear_drag, step);
        let angular = Integration::new(parameters.angular_drag, step);
        let angular_half = Integration::new(parameters.angular_drag, step * 0.5);
        let mut motion = Motion {
            force_squared_time: force * force * parameters.time_step,
            torque_squared_time: torque * torque * parameters.time_step,
            ..Motion::default()
        };

        self.confine(world);
        for _ in 0..parameters.substeps {
            let previous = self.position;
            let previous_velocity = self.velocity;
            let previous_angular_velocity = self.angular_velocity;
            let midpoint_heading = self.heading
                + angular_half.displacement(self.angular_velocity, angular_acceleration);
            let rotation = angular.displacement(self.angular_velocity, angular_acceleration);
            let next_heading = self.heading + rotation;
            ensure!(
                midpoint_heading.is_finite() && next_heading.is_finite(),
                "non-finite animal rotation"
            );
            let forward = direction(midpoint_heading);
            let applied_acceleration = acceleration * forward;
            let displacement =
                linear.travel * self.velocity + linear.acceleration_travel * applied_acceleration;
            self.velocity = linear.decay * self.velocity + linear.travel * applied_acceleration;
            self.angular_velocity = angular.velocity(self.angular_velocity, angular_acceleration);
            self.heading = normalize_heading(next_heading);
            ensure!(
                self.velocity.is_finite() && self.angular_velocity.is_finite(),
                "non-finite animal velocity"
            );
            self.position += displacement;
            ensure!(self.position.is_finite(), "non-finite animal displacement");
            self.confine(world);
            let actual_displacement = self.position - previous;
            motion.distance += actual_displacement.length();
            let forward_work = if force == 0.0 {
                0.0
            } else if let Some(start) = positive_work_start(
                forward.dot(previous_velocity),
                acceleration,
                parameters.linear_drag,
                step,
            ) {
                // Clamp the turning point as well as the endpoint: blocked travel
                // performs no work. Contact timing remains a substep approximation.
                let until_start = Integration::new(parameters.linear_drag, start);
                let turning_position = previous
                    + until_start.travel * previous_velocity
                    + until_start.acceleration_travel * applied_acceleration;
                ensure!(
                    turning_position.is_finite(),
                    "non-finite animal turning position"
                );
                let limit = (world.half_size() - self.radius()).max(DVec2::ZERO);
                force * forward.dot(self.position - turning_position.clamp(-limit, limit))
            } else {
                0.0
            };
            let turning_work = if torque == 0.0 {
                0.0
            } else if let Some(start) = positive_work_start(
                previous_angular_velocity * torque.signum(),
                angular_acceleration.abs(),
                parameters.angular_drag,
                step,
            ) {
                let positive_rotation = if start == 0.0 {
                    rotation * torque.signum()
                } else {
                    Integration::new(parameters.angular_drag, step - start)
                        .displacement(0.0, angular_acceleration.abs())
                };
                torque.abs() * positive_rotation
            } else {
                0.0
            };
            ensure!(
                forward_work.is_finite() && turning_work.is_finite(),
                "non-finite animal mechanical work"
            );
            motion.mechanical_work += forward_work.max(0.0) + turning_work.max(0.0);
        }
        ensure!(
            motion.distance.is_finite()
                && motion.mechanical_work.is_finite()
                && motion.force_squared_time.is_finite()
                && motion.torque_squared_time.is_finite(),
            "non-finite integrated animal motion"
        );
        self.validate(world, parameters)?;
        Ok(motion)
    }
}

fn direction(heading: f64) -> DVec2 {
    let (sin, cos) = heading.sin_cos();
    DVec2::new(cos, sin)
}

fn normalize_heading(heading: f64) -> f64 {
    let wrapped = heading.rem_euclid(TAU);
    if wrapped >= PI {
        wrapped - TAU
    } else {
        wrapped
    }
}

/// Start of positive power under a constant force, with velocity projected along
/// that force. Opposing velocity crosses zero at most once under linear drag.
fn positive_work_start(velocity: f64, acceleration: f64, drag: f64, time: f64) -> Option<f64> {
    if velocity >= 0.0 {
        return Some(0.0);
    }
    if Integration::new(drag, time).velocity(velocity, acceleration) <= 0.0 {
        return None;
    }
    let undamped = -velocity / acceleration;
    let scaled = drag * undamped;
    let crossing = if scaled == 0.0 {
        undamped
    } else if scaled.is_finite() {
        undamped * (scaled.ln_1p() / scaled)
    } else {
        // The ratio may overflow even though its logarithm and crossing time fit.
        let log_scaled = drag.ln() + (-velocity).ln() - acceleration.ln();
        let logarithm = if log_scaled > f64::MAX.ln() {
            log_scaled
        } else {
            log_scaled.exp().ln_1p()
        };
        logarithm / drag
    };
    Some(crossing.clamp(0.0, time))
}

/// Exact coefficients for constant acceleration with velocity-proportional drag.
struct Integration {
    decay: f64,
    travel: f64,
    acceleration_travel: f64,
}

impl Integration {
    fn new(drag: f64, time: f64) -> Self {
        let z = drag * time;
        let (travel, acceleration_travel) = if z < 1e-3 {
            // Series avoid subtracting nearly equal terms, including at zero drag.
            let travel =
                time * (1.0 + z * (-0.5 + z * (1.0 / 6.0 + z * (-1.0 / 24.0 + z / 120.0))));
            let acceleration_travel = time
                * time
                * (0.5 + z * (-1.0 / 6.0 + z * (1.0 / 24.0 + z * (-1.0 / 120.0 + z / 720.0))));
            (travel, acceleration_travel)
        } else {
            let travel = -(-z).exp_m1() / drag;
            (travel, (time - travel) / drag)
        };
        Self {
            decay: (-z).exp(),
            travel,
            acceleration_travel,
        }
    }

    fn velocity(&self, velocity: f64, acceleration: f64) -> f64 {
        self.decay * velocity + self.travel * acceleration
    }

    fn displacement(&self, velocity: f64, acceleration: f64) -> f64 {
        self.travel * velocity + self.acceleration_travel * acceleration
    }
}

#[cfg(test)]
mod tests;
