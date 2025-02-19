use std::sync::mpsc::Receiver;

use anyhow::Result;
use eframe::{self, egui, CreationContext};

use nevo_api::ViewMessage;

pub struct App {
    messages: Receiver<ViewMessage>,
}

impl App {
    pub fn new(_cc: &CreationContext, messages: Receiver<ViewMessage>) -> Result<Self> {
        Ok(Self { messages })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(msg) = self.messages.try_recv() {
            println!("View update received");
        }

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
