//! Fixed circular obstacles; overlapping circles describe their geometric union.

use anyhow::{Result, ensure};
use glam::Vec2;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};

use super::WorldConfig;

/// An immovable circle with infinite mass. Positions do not evolve with ticks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obstacle {
    pub position: Vec2,
    pub radius: f32,
}

/// Explicit geometry or a seeded, uniform distribution across the arena.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Config {
    Explicit(Vec<Obstacle>),
    Random(RandomConfig),
}

impl Default for Config {
    fn default() -> Self {
        Self::Explicit(Vec::new())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RandomConfig {
    pub count: usize,
    pub radius_min: f32,
    pub radius_max: f32,
}

impl Config {
    pub(super) fn validate(&self, world: WorldConfig) -> Result<()> {
        match self {
            Self::Explicit(obstacles) => validate(obstacles, world),
            Self::Random(config) => {
                ensure!(
                    config.radius_min.is_finite()
                        && config.radius_max.is_finite()
                        && config.radius_min > 0.0
                        && config.radius_min <= config.radius_max,
                    "random obstacle radii must be finite, positive, and ordered"
                );
                validate(
                    &[Obstacle {
                        position: Vec2::ZERO,
                        radius: config.radius_max,
                    }],
                    world,
                )?;
                ensure!(
                    config.count <= isize::MAX as usize / size_of::<Obstacle>(),
                    "obstacle count produces an oversized layout"
                );
                Ok(())
            }
        }
    }

    pub(super) fn generate<R: Rng + ?Sized>(
        &self,
        world: WorldConfig,
        rng: &mut R,
    ) -> Result<Vec<Obstacle>> {
        self.validate(world)?;
        match self {
            Self::Explicit(obstacles) => Ok(obstacles.clone()),
            Self::Random(config) => {
                let mut obstacles = Vec::new();
                obstacles.try_reserve_exact(config.count)?;
                for _ in 0..config.count {
                    let radius = rng.random_range(config.radius_min..=config.radius_max);
                    let limit = world.half_size() - Vec2::splat(radius);
                    obstacles.push(Obstacle {
                        position: super::patches::uniform_position(rng, limit),
                        radius,
                    });
                }
                Ok(obstacles)
            }
        }
    }

    pub(super) fn validate_layout(&self, obstacles: &[Obstacle], world: WorldConfig) -> Result<()> {
        self.validate(world)?;
        validate(obstacles, world)?;
        let matches = match self {
            Self::Explicit(expected) => obstacles == expected,
            Self::Random(config) => {
                obstacles.len() == config.count
                    && obstacles.iter().all(|obstacle| {
                        (config.radius_min..=config.radius_max).contains(&obstacle.radius)
                    })
            }
        };
        ensure!(matches, "stored obstacles differ from their configuration");
        Ok(())
    }
}

pub(super) fn validate(obstacles: &[Obstacle], world: WorldConfig) -> Result<()> {
    let half_size = world.half_size();
    ensure!(
        half_size.is_finite() && half_size.cmpgt(Vec2::ZERO).all(),
        "obstacle arena dimensions must be finite and positive"
    );
    for obstacle in obstacles {
        ensure!(
            obstacle.position.is_finite() && obstacle.radius.is_finite() && obstacle.radius > 0.0,
            "obstacles require finite centres and positive finite radii"
        );
        let limit = half_size - Vec2::splat(obstacle.radius);
        ensure!(
            obstacle.position.abs().cmple(limit).all(),
            "obstacle circles must fit entirely inside the arena"
        );
    }
    Ok(())
}

/// Test clearance from validated obstacles; touching circles do not overlap.
pub(super) fn clear(position: Vec2, radius: f32, obstacles: &[Obstacle]) -> bool {
    if !position.is_finite() || !radius.is_finite() || radius < 0.0 {
        return false;
    }
    obstacles.iter().all(|obstacle| {
        let separation = radius + obstacle.radius;
        let delta = position - obstacle.position;
        separation.is_finite() && delta.x.hypot(delta.y) >= separation
    })
}

#[cfg(feature = "viewer")]
impl crate::viewer::Draw for Obstacle {
    fn draw(&self, drawing: &mut crate::viewer::Drawing<'_>) {
        use wgame::{prelude::*, rgb::Rgb};

        let position = self.position;
        let radius = self.radius;
        let outer_radius = radius * 1.04;
        let circle = drawing.library.shapes().unit_circle();
        drawing.scene.add(
            &circle
                .fill_color(Rgb::new(0.24, 0.21, 0.17))
                .scale(outer_radius)
                .move_to(position)
                .order(0),
        );
        drawing.scene.add(
            &circle
                .fill_color(Rgb::new(0.43, 0.39, 0.32))
                .scale(radius)
                .move_to(position)
                .order(1),
        );
        drawing.include_bounds(crate::viewer::Bounds {
            min: position - Vec2::splat(outer_radius),
            max: position + Vec2::splat(outer_radius),
        });
    }
}

#[cfg(test)]
mod tests;
