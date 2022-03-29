use nevo::app::App;

fn main() {
    let app = App::default();
    eframe::run_native(Box::new(app), eframe::NativeOptions::default());
}
