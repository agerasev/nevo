use anyhow::Result;
use eframe::{self, CreationContext, egui};

use crate::world::view::WorldView;

pub struct App {
    world: WorldView,
}

impl App {
    pub fn new(_cc: &CreationContext, world: WorldView) -> Result<Self> {
        Ok(Self { world })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                ui.separator();
            });
        });

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| ui.heading("Windows"));
        });

        egui::CentralPanel::default().show(ctx, |_ui| {
            egui::Window::new("Map").show(ctx, |ui| {
                self.world.draw(ui);
            });
        });
    }
}
