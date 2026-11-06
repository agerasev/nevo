use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
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

use super::{Draw, Drawing, Label, controls::Controls, metrics};
use crate::run::{Command, Control, UniverseView};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    World,
    Metrics,
}

/// Window-owned information; model-dependent labels come from `Draw`.
pub struct Options {
    pub title: String,
    pub labels: Vec<Label>,
    pub metrics_path: Option<PathBuf>,
    pub initial_tab: Tab,
    pub exit_requested: Arc<AtomicBool>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            title: "NEvo".into(),
            labels: Vec::new(),
            metrics_path: None,
            initial_tab: Tab::World,
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
    let active_tab = Cell::new(options.initial_tab);
    let metrics = RefCell::new(metrics::Panel::new(options.metrics_path.clone()));
    let initial = control.status();
    let controls = RefCell::new(Controls::new(initial.interval));
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
                ui.add_enabled_ui(!status.stopped, |ui| {
                    controls.borrow_mut().show(ui, status.paused, click);
                });
                ui.separator();
                for item in &options.labels {
                    label(ui, item);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if status.stopped {
                    ui.label("Simulation stopped");
                }
                for item in &status.labels {
                    label(ui, item);
                }
            });
            let mut tab = active_tab.get();
            tab_bar(ui, &mut tab);
            active_tab.set(tab);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| match active_tab.get() {
                Tab::World => canvas.show(ui),
                Tab::Metrics => {
                    egui::Frame::new().inner_margin(12.0).show(ui, |ui| {
                        metrics.borrow_mut().show(ui);
                    });
                    // The host still receives its stable canvas response, but
                    // zero geometry makes it invisible and relinquishes focus.
                    let mut hidden = ui.new_child(egui::UiBuilder::new().max_rect(
                        egui::Rect::from_min_size(ui.max_rect().min, egui::Vec2::ZERO),
                    ));
                    hidden.set_clip_rect(egui::Rect::NOTHING);
                    canvas.show(&mut hidden)
                }
            })
            .inner
    });
    let library = Library::new(host.graphics());
    loop {
        if options.exit_requested.load(Ordering::Relaxed) {
            return Ok(());
        }
        for command in pending.take() {
            // A completed simulation remains visible until the window closes.
            let _ = control.send(command);
        }
        metrics.borrow_mut().poll();
        let mut drawing = Drawing::new(&library);
        if active_tab.get() == Tab::World {
            state.read().draw(&mut drawing);
        }
        let loop_status = control.status();
        controls.borrow_mut().begin_frame(loop_status.interval);
        *status.borrow_mut() = Status {
            paused: loop_status.paused,
            stopped: loop_status.stop.is_some(),
            labels: std::mem::take(&mut drawing.labels),
        };
        // UI layout and GPU work use the collected scene after its state lock ends.
        // The window/backend schedules presentation; there is no extra frame timer.
        let Some(mut frame) = host.next_frame().await? else {
            return Ok(());
        };
        if loop_status.stop.is_none() && active_tab.get() == Tab::World {
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
    }
}

// The application owns tab selection; neither model drawing nor metrics loading
// interprets UI input or needs to know about the other view.
fn tab_bar(ui: &mut egui::Ui, tab: &mut Tab) -> [egui::Response; 2] {
    ui.horizontal(|ui| {
        [
            ui.selectable_value(tab, Tab::World, "World"),
            ui.selectable_value(tab, Tab::Metrics, "Metrics"),
        ]
    })
    .inner
}
