use std::sync::{Arc, RwLock};

use anyhow::Result;
use eframe::{self, egui, CreationContext};

use crate::world::World;

pub struct App {
    world: Arc<RwLock<World>>,
}

impl App {
    pub fn new(_cc: &CreationContext, world: Arc<RwLock<World>>) -> Result<Self> {
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
            egui::Window::new("Window").show(ctx, |ui| {
                ui.heading("My egui Application");
            });
        });
    }
}
