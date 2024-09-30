pub mod pipe;

use anyhow::Result;
use eframe::{self, egui, CreationContext};

pub struct App {
    name: String,
    age: u32,
}

impl App {
    pub fn new(_cc: &CreationContext) -> Result<Self> {
        Ok(Self {
            name: "Alex".to_owned(),
            age: 28,
        })
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
        });
    }
}
