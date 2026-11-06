//! Offscreen checks: `cargo test --locked --test viewer_rendering -- --ignored`.
//! Requires a GPU or software Vulkan adapter; explicit execution must not skip it.
#![cfg(feature = "viewer")]

#[allow(dead_code)]
mod support;

use std::{sync::mpsc, time::Duration};

use glam::{DVec2, Vec2};
use nevo::{
    ecosystem::{Animal, Body, Ecosystem, Plant},
    universe::Universe,
    viewer::{Bounds, Camera, Draw, Drawing},
};
use wgame::{
    Library,
    canvas::{CanvasInput, Event},
    gfx::{Graphics, Offscreen, Target},
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
        let plant = Plant::new(1, DVec2::new(-0.5, 0.0), 0.0001);
        let mut animal = Animal {
            body: Body::new(DVec2::new(0.5, 0.0), 0.0004),
            mind: UnrelatedMind,
        };
        animal.body.heading = std::f64::consts::FRAC_PI_2;
        plant.draw(&mut drawing);
        animal.draw(&mut drawing);
    }
    drawing.bounds = Some(Bounds {
        min: -Vec2::ONE,
        max: Vec2::ONE,
    });
    let mut target = Offscreen::new(&graphics, (100, 100));
    drawing.render(&mut target, &Camera::default());
    let image = pixels(&mut target);
    assert_pixel(&image, 26, 50, [51, 189, 82, 255]);
    assert_pixel(&image, 74, 50, [255, 217, 64, 255]);
    assert_pixel(&image, 74, 44, [46, 36, 10, 255]);
    assert_pixel(&image, 50, 50, [14, 18, 19, 255]);
    // Navigation uses the same drawing path, with zoom anchored at the animal.
    let mut camera = Camera::default();
    let mut input = CanvasInput::default();
    input.events = vec![
        Event::Moved(Vec2::new(74.0, 50.0)),
        Event::Scroll(Vec2::new(0.0, 2.0_f32.ln() / 0.005)),
    ];
    camera.update(drawing.bounds, (100.0, 100.0), &input);
    drawing.render(&mut target, &camera);
    let image = pixels(&mut target);
    assert_pixel(&image, 74, 50, [255, 217, 64, 255]);
    assert_pixel(&image, 88, 50, [255, 217, 64, 255]);
    assert_pixel(&image, 74, 38, [46, 36, 10, 255]);
    assert_pixel(&image, 26, 50, [14, 18, 19, 255]);
    // No genetic or inference bounds leak through the world's drawing interface.
    fn assert_draw<T: Draw>() {}
    assert_draw::<Ecosystem<UnrelatedMind, (), (), (), ()>>();
}

#[test]
#[ignore = "requires a graphics adapter"]
fn overlapping_circles_hide_borders_inside_their_fills() {
    let graphics = graphics();
    let library = Library::new(&graphics);
    let plant = |x| Plant::new(1, DVec2::new(x, 0.0), 0.0025);
    let body = |x| {
        let mut body = Body::new(DVec2::new(x, 0.0), 0.0025);
        // Keep the heading marker clear of the horizontal border checks.
        body.heading = std::f64::consts::FRAC_PI_2;
        body
    };
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
            drawing.render(&mut target, &Camera::default());
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

#[test]
#[ignore = "requires a graphics adapter"]
fn ecosystem_drawing_preserves_complete_state() {
    let graphics = graphics();
    let library = Library::new(&graphics);
    let mut universe = Universe::new(support::ecosystem("world/debug"), 1).unwrap();
    universe.advance().unwrap();
    let before = universe.clone();
    let mut drawing = Drawing::new(&library);
    universe.draw(&mut drawing);
    let mut target = Offscreen::new(&graphics, (100, 100));
    drawing.render(&mut target, &Camera::default());
    assert_eq!(universe, before);
}
