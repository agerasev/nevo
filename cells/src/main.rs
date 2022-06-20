mod app;
mod cell;
mod medium;

fn main() {
    let app = app::App::default();
    eframe::run_native(
        "Cells",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(app)),
    );
}
