use crate::world::World;
use eframe::{self, egui};
use nevo::core::{rand::SeedableRng, SmallRng};
use vecmat::vector::Vector2;

pub struct App {
    rng: SmallRng,
    world: World,

    name: String,
    age: u32,
}

impl App {
    pub fn new() -> Self {
        let mut rng = SmallRng::seed_from_u64(0xdeadbeef);
        Self {
            world: World::new(&mut rng, Vector2::from((64.0, 64.0)), 16),
            rng,

            name: "Alex".to_owned(),
            age: 28,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_style(egui::Style::default());

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Click each year").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::containers::Frame::dark_canvas(ui.style()).show(ui, |ui| {
                ui.ctx().request_repaint();
                self.world.draw(ui);
            });
        });
    }
}
