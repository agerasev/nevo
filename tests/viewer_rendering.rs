//! Offscreen checks: `cargo test --locked --test viewer_rendering -- --ignored`.
//! Requires a GPU or software Vulkan adapter; explicit execution must not skip it.
#![cfg(feature = "viewer")]

#[allow(dead_code)]
mod support;

use std::{sync::mpsc, thread, time::Duration};

use glam::{DVec2, Vec2};
use nevo::{
    ecosystem::{Animal, Body, Ecosystem, Plant},
    run::Run,
    universe::{Simulation, Universe},
    viewer::{Bounds, Draw, Drawing, Label},
};
use wgame::{
    Library,
    gfx::{Graphics, Offscreen, Target},
    prelude::*,
    rgb::Rgb,
};

fn graphics() -> Graphics {
    futures::executor::block_on(async {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("GPU adapter required (Mesa lavapipe works)");
        eprintln!("Adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .unwrap();
        Graphics::new(adapter, device, queue, wgpu::TextureFormat::Rgba8Unorm)
    })
}

fn pixels(target: &mut Offscreen) -> Vec<u8> {
    let (width, height) = target.size();
    let stride = (width * 4).div_ceil(256) * 256;
    let buffer = target
        .state()
        .device()
        .create_buffer(&wgpu::BufferDescriptor {
            label: Some("viewer readback"),
            size: (stride * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
    let texture = target.texture().clone();
    target.encoder().copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    target.submit();
    let (done, result) = mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| done.send(r).unwrap());
    target
        .state()
        .device()
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();
    result
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    let result = data
        .chunks(stride as usize)
        .flat_map(|row| row[..width as usize * 4].iter().copied())
        .collect();
    drop(data);
    buffer.unmap();
    result
}

fn assert_pixel(data: &[u8], x: usize, y: usize, expected: [u8; 4]) {
    let pixel = &data[(y * 100 + x) * 4..(y * 100 + x) * 4 + 4];
    for (actual, expected) in pixel.iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 2,
            "pixel at ({x}, {y}): {pixel:?}"
        );
    }
}

#[test]
#[ignore = "requires a graphics adapter"]
fn entities_collect_owned_geometry_without_mind_traits() {
    struct UnrelatedMind;
    let graphics = graphics();
    let library = Library::new(&graphics);
    let mut drawing = Drawing::new(&library);
    {
        let plant = Plant::new(1, DVec2::new(-0.5, 0.0), 0.04);
        let animal = Animal {
            body: Body::new(DVec2::new(0.5, 0.0), 0.16),
            mind: UnrelatedMind,
        };
        plant.draw(&mut drawing);
        animal.draw(&mut drawing);
    }
    drawing.bounds = Some(Bounds {
        min: -Vec2::ONE,
        max: Vec2::ONE,
    });
    let mut target = Offscreen::new(&graphics, (100, 100));
    drawing.render(&mut target);
    let image = pixels(&mut target);
    assert_pixel(&image, 26, 50, [51, 189, 82, 255]);
    assert_pixel(&image, 74, 50, [255, 217, 64, 255]);
    assert_pixel(&image, 50, 50, [14, 18, 19, 255]);
    // No genetic or inference bounds leak through the world's drawing interface.
    let world = Ecosystem::<UnrelatedMind, (), (), ()>::empty(0, 0, 1, 2, None, ());
    world.draw(&mut Drawing::new(&library));
}

#[test]
#[ignore = "requires a graphics adapter"]
fn overlapping_circles_hide_borders_inside_their_fills() {
    let graphics = graphics();
    let library = Library::new(&graphics);
    let plant = |x| Plant::new(1, DVec2::new(x, 0.0), 1.0);
    let body = |x| Body::new(DVec2::new(x, 0.0), 1.0);
    let green = [51, 189, 82, 255];
    let yellow = [255, 217, 64, 255];
    let cases: [(&dyn Draw, &dyn Draw, _, _); 3] = [
        (&plant(-0.25), &plant(0.25), green, green),
        (&body(-0.25), &body(0.25), yellow, yellow),
        (&plant(-0.25), &body(0.25), green, yellow),
    ];
    for (left, right, left_color, right_color) in cases {
        // Reverse collection too: layering must not depend on traversal order.
        for objects in [[left, right], [right, left]] {
            let mut drawing = Drawing::new(&library);
            for object in objects {
                object.draw(&mut drawing);
            }
            drawing.bounds = Some(Bounds {
                min: -Vec2::ONE,
                max: Vec2::ONE,
            });
            let mut target = Offscreen::new(&graphics, (100, 100));
            drawing.render(&mut target);
            let image = pixels(&mut target);
            // The neighboring border crosses these interiors in a stroke-based draw.
            assert_pixel(&image, 37, 50, left_color);
            assert_pixel(&image, 62, 50, right_color);
            assert_pixel(&image, 26, 50, left_color);
            assert_pixel(&image, 74, 50, right_color);
            let border = if left_color == green {
                [20, 89, 38, 255]
            } else {
                [122, 89, 20, 255]
            };
            assert_pixel(&image, 13, 50, border);
            assert_pixel(&image, 10, 50, [14, 18, 19, 255]);
        }
    }
}

// A second world with no Clone implementation, no ecosystem entities, and offset bounds.
struct Marker {
    x: f32,
}

impl Simulation for Marker {
    type Config = ();
    type Update = f32;
    type Report = ();

    fn compute_update(&self, _: u64) -> anyhow::Result<((), f32)> {
        Ok(((), self.x + 1.0))
    }

    fn apply_update(&mut self, x: f32) {
        self.x = x;
    }
}

impl Draw for Marker {
    fn draw(&self, drawing: &mut Drawing<'_>) {
        drawing.bounds = Some(Bounds {
            min: Vec2::new(10.0, 20.0),
            max: Vec2::new(12.0, 22.0),
        });
        drawing.scene.add(
            &drawing
                .library
                .shapes()
                .unit_circle()
                .fill_color(Rgb::new(1.0, 0.0, 0.0))
                .scale(0.15)
                .move_to(Vec2::new(self.x, 21.0)),
        );
        drawing.labels.push(Label::new("Marker"));
    }
}

#[test]
#[ignore = "requires a graphics adapter"]
fn a_second_world_can_advance_while_its_previous_drawing_is_rendered() {
    let graphics = graphics();
    let library = Library::new(&graphics);
    let mut run = Run::from_universe(Universe::from_world(Marker { x: 10.5 }).unwrap());
    let state = run.universe().clone();
    let mut drawing = Drawing::new(&library);
    state.read().draw(&mut drawing);
    let (done, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        run.tick().unwrap();
        done.send(run).unwrap();
    });
    let run = result
        .recv_timeout(Duration::from_secs(3))
        .expect("drawing retained the simulation lock");
    worker.join().unwrap();
    assert_eq!(run.universe().read().world().x, 11.5);
    assert_eq!(drawing.labels, [Label::new("Tick 0"), Label::new("Marker")]);
    let mut target = Offscreen::new(&graphics, (100, 100));
    drawing.render(&mut target);
    let image = pixels(&mut target);
    assert_pixel(&image, 26, 50, [255, 0, 0, 255]);
    assert_pixel(&image, 74, 50, [14, 18, 19, 255]);
    let mut next = Drawing::new(&library);
    state.read().draw(&mut next);
    next.render(&mut target);
    let image = pixels(&mut target);
    assert_pixel(&image, 26, 50, [14, 18, 19, 255]);
    assert_pixel(&image, 74, 50, [255, 0, 0, 255]);
    // The normal window entry point also accepts this non-Clone world.
    let _ = nevo::viewer::show::<Universe<Marker>>;
}

#[test]
#[ignore = "requires a graphics adapter"]
fn ecosystem_drawing_preserves_complete_state_and_same_tick_labels() {
    let graphics = graphics();
    let library = Library::new(&graphics);
    for config in [
        support::ecosystem::<nevo::ecosystem::Config>("debug"),
        support::ecosystem::<nevo::ecosystem::Config>("baseline"),
    ] {
        for seed in [1, 42] {
            let mut universe = Universe::new(config.clone(), seed).unwrap();
            for tick in 0..3 {
                let before = universe.clone();
                let mut drawing = Drawing::new(&library);
                universe.draw(&mut drawing);
                assert_eq!(drawing.labels[0].text, format!("Tick {tick}"));
                assert_eq!(
                    drawing.labels[1].text,
                    format!("Plants {}", universe.world().plants().len())
                );
                assert_eq!(
                    drawing.labels[2].text,
                    format!("Animals {}", universe.world().animals().len())
                );
                let mut target = Offscreen::new(&graphics, (100, 100));
                drawing.render(&mut target);
                assert!(!pixels(&mut target).is_empty());
                assert_eq!(
                    universe, before,
                    "drawing changed model state or consumed randomness"
                );
                universe.advance().unwrap();
            }
        }
    }
}
