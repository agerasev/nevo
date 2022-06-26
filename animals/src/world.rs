use crate::animal::{Animal, Genome};
use eframe::{
    self,
    egui::{self, pos2},
    emath, epaint,
};
use nevo::core::rand::{distributions::Uniform, Rng};
use vecmat::vector::Vector2;

pub struct World {
    size: Vector2<f64>,
    animals: Vec<Animal>,
}

impl World {
    const ANIMAL_INIT_MEM_SIZE: usize = 4;

    pub fn new<R: Rng>(rng: &mut R, size: Vector2<f64>, count: usize) -> Self {
        let mut animals = Vec::new();
        for _ in 0..count {
            animals.push(Animal::new(
                Genome::new(Self::ANIMAL_INIT_MEM_SIZE, 1.0, rng),
                size * Vector2::init(|| rng.sample(Uniform::from(0.0..=1.0))),
            ));
        }
        Self { size, animals }
    }

    pub fn step(&mut self) {}

    pub fn draw(&self, ui: &mut egui::Ui) {
        let size = {
            let s = ui.available_size();
            f32::min(s.x, s.y)
        };
        let (_id, rect) = ui.allocate_space(egui::Vec2::new(size, size));

        let to_screen = emath::RectTransform::from_to(
            egui::Rect::from_x_y_ranges(0.0..=(self.size.x() as f32), 0.0..=(self.size.y() as f32)),
            rect,
        );

        let mut shapes = Vec::new();
        for animal in &self.animals {
            animal.draw_shapes(&mut shapes, to_screen);
        }
        ui.painter().extend(shapes);
    }
}
