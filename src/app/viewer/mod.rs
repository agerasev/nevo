pub(super) mod smoke;

use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nevo::{
    environment::ecosystem::{Ecosystem, HALF_SIZE, Scenario},
    run::{Command, Control, UniverseView},
    universe::Universe,
};
use wgame::{
    ContentFrame, Library, Window, WindowHost,
    canvas::{Event, Key},
    prelude::*,
    rgb::Rgb,
};
use wgame_egui::{EguiWindow, egui};

use self::smoke::SmokeCheck;

const PLANT: Rgb<f32> = Rgb::new(0.20, 0.74, 0.32);
const HERBIVORE: Rgb<f32> = Rgb::new(1.0, 0.85, 0.25);

#[derive(Clone, Copy)]
struct Status {
    paused: bool,
    stopped: bool,
    tick: u64,
    plants: usize,
    animals: usize,
}

impl Status {
    fn read(universe: &Universe, control: &Control) -> Self {
        let world = universe.environment();
        let status = control.status();
        Self {
            paused: status.paused,
            stopped: status.stop.is_some(),
            tick: universe.step(),
            plants: world.plants().len(),
            animals: world.animals().len(),
        }
    }
}

fn draw_world(frame: &mut impl ContentFrame, library: &Library, world: &Ecosystem) {
    frame.clear(Rgb::new(0.055, 0.07, 0.075));
    if !frame.visible() {
        return;
    }
    let half = HALF_SIZE.as_vec2();
    let (width, height) = frame.size();
    let aspect = width as f32 / height as f32;
    let scale = (aspect / half.x).min(1.0 / half.y) * 0.96;
    let mut scene = frame.scene();
    scene.camera = scene.camera.scale(scale);
    scene.add(
        &library
            .shapes()
            .rectangle((-half, half))
            .fill_color(Rgb::new(0.10, 0.18, 0.13))
            .order(-1),
    );
    let circle = library.shapes().unit_circle();
    // Stroke expansion must happen before translation, so it cannot move the center.
    for plant in world.plants() {
        let position = plant.position.as_vec2();
        let radius = plant.radius() as f32;
        scene.add(
            &circle
                .fill_color(PLANT)
                .scale(radius)
                .move_to(position)
                .order(0),
        );
        scene.add(
            &circle
                .stroke_color(0.10, Rgb::new(0.08, 0.35, 0.15))
                .scale(radius)
                .move_to(position)
                .order(1),
        );
    }
    for body in world.animals().values() {
        let position = body.position.as_vec2();
        let radius = body.radius() as f32;
        scene.add(
            &circle
                .fill_color(HERBIVORE)
                .scale(radius)
                .move_to(position)
                .order(2),
        );
        scene.add(
            &circle
                .stroke_color(0.14, Rgb::new(0.48, 0.35, 0.08))
                .scale(radius)
                .move_to(position)
                .order(3),
        );
    }
    scene.render();
}

pub async fn show(
    window: Window<'_>,
    world: UniverseView<Universe>,
    control: Control,
    scenario: Scenario,
    seed: u64,
    mut check: Option<SmokeCheck>,
    exit_requested: Arc<AtomicBool>,
) -> wgame::Result<()> {
    let initial_snapshot = world.read().clone();
    let status = Cell::new(Status::read(&initial_snapshot, &control));
    let pending: RefCell<Vec<Command>> = RefCell::default();
    // egui can retry a layout; latch one command for a click across its passes.
    let click = |command| {
        let mut pending = pending.borrow_mut();
        if !pending.contains(&command) {
            pending.push(command);
        }
    };
    let mut host = EguiWindow::new(window, |ui, canvas| {
        let status = status.get();
        // A held key activates a focused toolbar button only once.
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
            })
        });
        egui::Panel::top("controls").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("NEvo · Foraging");
                ui.separator();
                if ui
                    .add_enabled(
                        !status.stopped,
                        egui::Button::new(if status.paused { "Resume" } else { "Pause" }),
                    )
                    .on_hover_text("Space when the arena has focus")
                    .clicked()
                {
                    click(Command::TogglePause);
                }
                if ui
                    .add_enabled(!status.stopped, egui::Button::new("Single step"))
                    .on_hover_text("Pause and advance one tick (Right Arrow in the arena)")
                    .clicked()
                {
                    click(Command::Step);
                }
                ui.separator();
                ui.label(format!("{scenario:?}"));
                ui.label(format!("Seed {seed}"));
            });
            ui.horizontal_wrapped(|ui| {
                ui.monospace(format!("Tick {}", status.tick));
                if status.stopped {
                    ui.label("Stopped");
                }
                ui.separator();
                ui.colored_label(
                    egui::Color32::from_rgb(85, 205, 112),
                    format!("Plants {}", status.plants),
                );
                ui.colored_label(
                    egui::Color32::from_rgb(250, 212, 68),
                    format!("Herbivores {}", status.animals),
                );
            });
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| canvas.show(ui))
            .inner
    });
    let library = Library::new(host.graphics());
    let mut initial = true;
    let mut smoke_frame = 0;
    loop {
        if exit_requested.load(Ordering::Relaxed) {
            return Ok(());
        }
        if !initial && pending.borrow().is_empty() {
            host.wait_for_update(Some(Duration::from_millis(16))).await;
        }
        initial = false;
        if let Some(check) = &mut check {
            check.command(smoke_frame, &control)?;
        } else {
            for command in pending.take() {
                // A completed experiment remains visible until the window closes.
                let _ = control.send(command);
            }
        }

        // The lock covers the copy only. Layout and GPU work use one immutable tick.
        let snapshot = world.read().clone();
        status.set(Status::read(&snapshot, &control));
        let Some(mut frame) = host.next_frame().await? else {
            if check.is_some() {
                return Err(wgame::Error::msg(
                    "viewer closed before the smoke check finished",
                ));
            }
            return Ok(());
        };
        if check.is_none() && control.status().stop.is_none() {
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
        draw_world(&mut frame, &library, snapshot.environment());
        frame.present();
        if let Some(check) = &mut check {
            check.verify(smoke_frame, &snapshot, &control)?;
            smoke_frame += 1;
            if smoke_frame == 12 {
                return Ok(());
            }
        }
    }
}
