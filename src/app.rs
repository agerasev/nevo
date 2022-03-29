use eframe::{egui, epi};

pub struct App {
    name: String,
    age: u32,
}

impl Default for App {
    fn default() -> Self {
        Self {
            name: "Alex".to_owned(),
            age: 28,
        }
    }
}

impl epi::App for App {
    fn name(&self) -> &str {
        "NEvo"
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
        ctx.set_style(egui::Style::default());

        egui::CentralPanel::default().show(ctx, |ui| {
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
    }
}
