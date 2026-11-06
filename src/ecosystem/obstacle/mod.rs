//! Fixed circular obstacles; overlapping circles describe their geometric union.

use anyhow::{Result, ensure};
use glam::DVec2;
use serde::{Deserialize, Serialize};

use super::WorldConfig;

/// An immovable circle with infinite mass. Positions do not evolve with ticks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obstacle {
    pub position: DVec2,
    pub radius: f64,
}

pub(super) fn validate(obstacles: &[Obstacle], world: WorldConfig) -> Result<()> {
    let half_size = world.half_size();
    ensure!(
        half_size.is_finite() && half_size.cmpgt(DVec2::ZERO).all(),
        "obstacle arena dimensions must be finite and positive"
    );
    for obstacle in obstacles {
        ensure!(
            obstacle.position.is_finite() && obstacle.radius.is_finite() && obstacle.radius > 0.0,
            "obstacles require finite centres and positive finite radii"
        );
        let limit = half_size - DVec2::splat(obstacle.radius);
        ensure!(
            obstacle.position.abs().cmple(limit).all(),
            "obstacle circles must fit entirely inside the arena"
        );
    }
    Ok(())
}

/// Test clearance from validated obstacles; touching circles do not overlap.
pub(super) fn clear(position: DVec2, radius: f64, obstacles: &[Obstacle]) -> bool {
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
        use glam::Vec2;
        use wgame::{prelude::*, rgb::Rgb};

        let position = self.position.as_vec2();
        let radius = self.radius as f32;
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
