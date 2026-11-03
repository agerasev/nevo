use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

use nevo::foraging::{ForagingWorld, HALF_SIZE, Scenario};
use wgame::{
    ContentFrame, Library, Window, WindowHost,
    app::time::Instant,
    canvas::{Event, Key},
    prelude::*,
    rgb::Rgb,
};
use wgame_egui::{EguiWindow, egui};

use crate::{
    playback::{Command, Playback},
    smoke::SmokeCheck,
};

const PLANT: Rgb<f32> = Rgb::new(0.20, 0.74, 0.32);
const HERBIVORE: Rgb<f32> = Rgb::new(1.0, 0.85, 0.25);

#[derive(Clone, Copy)]
struct Status {
    paused: bool,
    tick: u64,
    plants: usize,
    animals: usize,
}

impl Status {
    fn read(playback: &Playback) -> Self {
        let world = &playback.run.world;
        Self {
            paused: playback.paused(),
            tick: world.number(),
            plants: world.plants().len(),
            animals: world.animals().len(),
        }
    }
}

fn draw_world(frame: &mut impl ContentFrame, library: &Library, world: &ForagingWorld) {
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
    for plant in world.plants() {
        let shape = circle
            .scale(plant.radius() as f32)
            .move_to(plant.position.as_vec2());
        scene.add(&shape.fill_color(PLANT).order(0));
        scene.add(
            &shape
                .stroke_color(0.10, Rgb::new(0.08, 0.35, 0.15))
                .order(1),
        );
    }
    for body in world.animals().values() {
        let shape = circle
            .scale(body.radius() as f32)
            .move_to(body.position.as_vec2());
        scene.add(&shape.fill_color(HERBIVORE).order(2));
        scene.add(
            &shape
                .stroke_color(0.14, Rgb::new(0.48, 0.35, 0.08))
                .order(3),
        );
    }
    scene.render();
}

pub async fn show(
    window: Window<'_>,
    scenario: Scenario,
    seed: u64,
    paused: bool,
    smoke: bool,
) -> wgame::Result<()> {
    let mut playback = Playback::new(scenario, seed, paused);
    let mut check = smoke.then(|| SmokeCheck::new(scenario, seed, paused));
    let status = Cell::new(Status::read(&playback));
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
                    .button(if status.paused { "Resume" } else { "Pause" })
                    .on_hover_text("Space when the arena has focus")
                    .clicked()
                {
                    click(Command::TogglePause);
                }
                if ui
                    .button("Single step")
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
    let mut last_update = Instant::now();
    let mut initial = true;
    let mut smoke_frame = 0;
    loop {
        if !initial && check.is_none() && pending.borrow().is_empty() {
            let timeout = playback
                .wait_duration()
                .map(|until_tick| until_tick.saturating_sub(last_update.elapsed()));
            host.wait_for_update(timeout).await;
        }
        let now = Instant::now();
        let (elapsed, commands) = match &mut check {
            Some(check) => check.frame(smoke_frame),
            None => (
                if initial {
                    Duration::ZERO
                } else {
                    now - last_update
                },
                pending.take(),
            ),
        };
        initial = false;
        last_update = now;
        playback.advance(elapsed, &commands);

        // Layout snapshots the advanced tick before the arena draws that same state.
        status.set(Status::read(&playback));
        let Some(mut frame) = host.next_frame().await? else {
            if smoke {
                return Err(wgame::Error::msg(
                    "viewer closed before the smoke check finished",
                ));
            }
            return Ok(());
        };
        if check.is_none() {
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
        draw_world(&mut frame, &library, &playback.run.world);
        frame.present();
        if let Some(check) = &mut check {
            check.verify(smoke_frame, &playback)?;
            smoke_frame += 1;
            if smoke_frame == 12 {
                return Ok(());
            }
        }
    }
}
