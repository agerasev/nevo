mod app;
mod bacteria;

fn main() {
    let app = app::App::default();
    eframe::run_native(Box::new(app), eframe::NativeOptions::default());
}
