use glam::Vec2;
use wgame::{prelude::*, rgb::Rgb};

use super::{Animal, Body, Ecosystem, Plant};
use crate::viewer::{Bounds, Draw, Drawing, Label};

const PLANT: Rgb<f32> = Rgb::new(0.20, 0.74, 0.32);
const ANIMAL: Rgb<f32> = Rgb::new(1.0, 0.85, 0.25);

impl Draw for Plant {
    fn draw(&self, drawing: &mut Drawing<'_>) {
        let position = self.position.as_vec2();
        let radius = self.radius() as f32;
        let outer_radius = radius * 1.05;
        let circle = drawing.library.shapes().unit_circle();
        // All borders sit below all fills, including other objects' fills.
        drawing.scene.add(
            &circle
                .fill_color(Rgb::new(0.08, 0.35, 0.15))
                .scale(outer_radius)
                .move_to(position)
                .order(0),
        );
        drawing.scene.add(
            &circle
                .fill_color(PLANT)
                .scale(radius)
                .move_to(position)
                .order(1),
        );
        drawing.include_bounds(Bounds {
            min: position - Vec2::splat(outer_radius),
            max: position + Vec2::splat(outer_radius),
        });
    }
}

impl Draw for Body {
    fn draw(&self, drawing: &mut Drawing<'_>) {
        let position = self.position.as_vec2();
        let radius = self.radius() as f32;
        let outer_radius = radius * 1.07;
        let circle = drawing.library.shapes().unit_circle();
        drawing.scene.add(
            &circle
                .fill_color(Rgb::new(0.48, 0.35, 0.08))
                .scale(outer_radius)
                .move_to(position)
                .order(0),
        );
        drawing.scene.add(
            &circle
                .fill_color(ANIMAL)
                .scale(radius)
                .move_to(position)
                .order(2),
        );
        drawing.include_bounds(Bounds {
            min: position - Vec2::splat(outer_radius),
            max: position + Vec2::splat(outer_radius),
        });
    }
}

impl<B: Draw, M> Draw for Animal<B, M> {
    fn draw(&self, drawing: &mut Drawing<'_>) {
        self.body.draw(drawing);
    }
}

impl<M, G, P, MC, IC> Draw for Ecosystem<M, G, P, MC, IC> {
    fn draw(&self, drawing: &mut Drawing<'_>) {
        let half = self.world_config().half_size().as_vec2();
        drawing.scene.add(
            &drawing
                .library
                .shapes()
                .rectangle((-half, half))
                .fill_color(Rgb::new(0.10, 0.18, 0.13))
                .order(-1),
        );
        for plant in self.plants() {
            plant.draw(drawing);
        }
        for animal in self.animals().values() {
            animal.draw(drawing);
        }
        // The arena, rather than the current population, determines the world view.
        drawing.bounds = Some(Bounds {
            min: -half,
            max: half,
        });
        drawing.labels.push(Label::colored(
            format!("Plants {}", self.plants().len()),
            PLANT,
        ));
        drawing.labels.push(Label::colored(
            format!("Animals {}", self.animals().len()),
            ANIMAL,
        ));
    }
}
