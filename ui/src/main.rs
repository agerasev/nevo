use std::{
    net::TcpStream,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::Duration,
};

use eframe::{self, egui};

use nevo_api::{ControlMessage, MessageReader, MessageWriter};
use nevo_ui::App;

fn main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Nevo Viewer",
        native_options,
        Box::new(|cc| {
            let (sender, receiver) = channel();
            let ctx = cc.egui_ctx.clone();

            let addr = ("localhost", 3399);
            let socket = TcpStream::connect(addr)
                .unwrap_or_else(|e| panic!("Cannot connect to {addr:?}: {e}"));
            spawn({
                let mut reader = MessageReader::new(socket.try_clone().unwrap());
                move || loop {
                    let msg = reader
                        .read_message()
                        .expect("Socket read error")
                        .expect("Socket closed");
                    sender.send(msg).unwrap();
                    ctx.request_repaint();
                }
            });
            spawn(move || {
                let mut writer = MessageWriter::new(socket);
                loop {
                    writer.write_message(&ControlMessage::Show).unwrap();
                    sleep(Duration::from_secs(4));
                }
            });

            Ok(Box::new(App::new(cc, receiver)?))
        }),
    )
    .unwrap();
}
