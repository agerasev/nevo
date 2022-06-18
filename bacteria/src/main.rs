mod app;
mod bacteria;
mod medium;

fn main() {
    let app = app::App::default();
    eframe::run_native(Box::new(app), eframe::NativeOptions::default());
}
