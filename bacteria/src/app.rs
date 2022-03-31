use eframe::{
    egui::{self, pos2},
    emath, epaint, epi,
};

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
        "Bacteria"
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &epi::Frame) {
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
            let color = egui::Color32::from_additive_luminance(196);
            egui::containers::Frame::dark_canvas(ui.style()).show(ui, |ui| {
                ui.ctx().request_repaint();
                let (_id, rect) = ui.allocate_space(ui.available_size());

                let to_screen = emath::RectTransform::from_to(
                    egui::Rect::from_x_y_ranges(0.0..=1.0, -1.0..=1.0),
                    rect,
                );
                let mut shapes = vec![];
                for &mode in &[2, 3, 5] {
                    let mode = mode as f64;
                    let n = 120;
                    let speed = 1.5;
                    let points: Vec<egui::Pos2> = (0..=n)
                        .map(|i| {
                            let t = i as f64 / (n as f64);
                            let amp = (ui.input().time * speed * mode).sin() / mode;
                            let y = amp * (t * std::f64::consts::TAU / 2.0 * mode).sin();
                            to_screen * pos2(t as f32, y as f32)
                        })
                        .collect();
                    let thickness = 10.0 / mode as f32;
                    shapes.push(epaint::Shape::line(
                        points,
                        egui::Stroke::new(thickness, color),
                    ));
                }
                ui.painter().extend(shapes);
            });
        });
    }
}
