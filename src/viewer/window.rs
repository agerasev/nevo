use std::{
    cell::RefCell,
    ops::ControlFlow,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use wgame::{
    ContentFrame, Library, Window, WindowHost,
    canvas::{Event, Key},
};
use wgame_egui::{EguiWindow, egui};

use super::{Draw, Drawing, Label};
use crate::run::{Command, Control, UniverseView};

/// Window-owned information; model-dependent labels come from `Draw`.
pub struct Options {
    pub title: String,
    pub labels: Vec<Label>,
    pub interactive: bool,
    pub exit_requested: Arc<AtomicBool>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            title: "NEvo".into(),
            labels: Vec::new(),
            interactive: true,
            exit_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct Status {
    paused: bool,
    stopped: bool,
    labels: Vec<Label>,
}

fn label(ui: &mut egui::Ui, label: &Label) {
    if let Some(color) = label.color {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        ui.colored_label(
            egui::Color32::from_rgb(byte(color.r), byte(color.g), byte(color.b)),
            &label.text,
        );
    } else {
        ui.monospace(&label.text);
    }
}

/// View any drawable state without requiring it to implement `Clone` or Simulation.
pub async fn show<U: Draw>(
    window: Window<'_>,
    state: UniverseView<U>,
    control: Control,
    options: Options,
) -> wgame::Result<()> {
    show_with(window, control, options, |drawing| {
        state.read().draw(drawing);
        Ok(ControlFlow::Continue(()))
    })
    .await
    .map(|_| ())
}

/// Supply a custom capture, e.g. to check state or script controls before drawing.
/// The callback runs once per attempted frame, outside UI layout. Release all
/// simulation guards before returning. `Break` presents this final drawing and
/// exits. The returned count includes only frames actually presented.
pub async fn show_with(
    window: Window<'_>,
    control: Control,
    options: Options,
    mut capture: impl FnMut(&mut Drawing<'_>) -> wgame::Result<ControlFlow<()>>,
) -> wgame::Result<u64> {
    let initial = control.status();
    let status = RefCell::new(Status {
        paused: initial.paused,
        stopped: initial.stop.is_some(),
        labels: Vec::new(),
    });
    let pending: RefCell<Vec<Command>> = RefCell::default();
    // egui can retry layout; latch each command once across those passes.
    let click = |command| {
        let mut pending = pending.borrow_mut();
        if !pending.contains(&command) {
            pending.push(command);
        }
    };
    let mut host = EguiWindow::new(window, |ui, canvas| {
        let status = status.borrow();
        ui.ctx().input_mut(|input| {
            input.events.retain(|event| {
                !matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Space | egui::Key::Enter,
                        pressed: true,
                        repeat: true,
                        ..
                    }
                )
            });
        });
        egui::Panel::top("controls").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(&options.title);
                ui.separator();
                if ui
                    .add_enabled(
                        options.interactive && !status.stopped,
                        egui::Button::new(if status.paused { "Resume" } else { "Pause" }),
                    )
                    .on_hover_text("Space when the arena has focus")
                    .clicked()
                {
                    click(Command::TogglePause);
                }
                if ui
                    .add_enabled(
                        options.interactive && !status.stopped,
                        egui::Button::new("Single step"),
                    )
                    .on_hover_text("Pause and advance one tick (Right Arrow in the arena)")
                    .clicked()
                {
                    click(Command::Step);
                }
                ui.separator();
                for item in &options.labels {
                    label(ui, item);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if status.stopped {
                    ui.label("Stopped");
                }
                for item in &status.labels {
                    label(ui, item);
                }
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| canvas.show(ui))
            .inner
    });
    let library = Library::new(host.graphics());
    let mut presented = 0;
    loop {
        if options.exit_requested.load(Ordering::Relaxed) {
            return Ok(presented);
        }
        for command in pending.take() {
            // A completed simulation remains visible until the window closes.
            let _ = control.send(command);
        }
        let mut drawing = Drawing::new(&library);
        let finish = capture(&mut drawing)?;
        let loop_status = control.status();
        *status.borrow_mut() = Status {
            paused: loop_status.paused,
            stopped: loop_status.stop.is_some(),
            labels: std::mem::take(&mut drawing.labels),
        };
        // UI layout and GPU work use the collected scene after its state lock ends.
        // The window/backend schedules presentation; there is no extra frame timer.
        let Some(mut frame) = host.next_frame().await? else {
            return Ok(presented);
        };
        if options.interactive && loop_status.stop.is_none() {
            for event in &frame.input().events {
                if let Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                } = event
                {
                    match key {
                        Key::Space => pending.borrow_mut().push(Command::TogglePause),
                        Key::ArrowRight => pending.borrow_mut().push(Command::Step),
                        _ => {}
                    }
                }
            }
        }
        if frame.visible() {
            drawing.render(&mut frame);
        }
        frame.present();
        presented += 1;
        if finish.is_break() {
            return Ok(presented);
        }
    }
}
