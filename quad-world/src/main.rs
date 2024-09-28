mod animal;
mod app;
mod world;

fn main() {
    let app = app::App::new();
    eframe::run_native(
        "Cells",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(app)),
    );
}
