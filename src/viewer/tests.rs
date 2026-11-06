use glam::{Vec2, Vec3};
use wgame::canvas::{Button, CanvasInput, Event};

use super::{Bounds, Camera};

fn input(events: &[Event]) -> CanvasInput {
    let mut input = CanvasInput::default();
    input.events = events.to_vec();
    input
}

// Convert canvas pixels through the actual rendering transform (Y points up).
fn world_at(camera: &Camera, bounds: Bounds, size: (u32, u32), point: Vec2) -> Vec2 {
    let logical = Vec3::new(
        (2.0 * point.x - size.0 as f32) / size.1 as f32,
        1.0 - 2.0 * point.y / size.1 as f32,
        0.0,
    );
    camera
        .transform(Some(bounds), size)
        .unwrap()
        .inverse()
        .transform_point3(logical)
        .truncate()
}

fn bounds() -> Bounds {
    Bounds {
        min: Vec2::new(-600.0, -300.0),
        max: Vec2::new(600.0, 300.0),
    }
}

#[test]
fn camera_centers_offset_bounds_and_preserves_aspect_with_padding() {
    let bounds = Bounds {
        min: Vec2::new(10.0, 20.0),
        max: Vec2::new(18.0, 24.0),
    };
    for (width, height) in [(400, 200), (200, 400), (200, 200)] {
        let transform = Camera::default()
            .transform(Some(bounds), (width, height))
            .unwrap();
        assert!(
            transform
                .transform_point3(Vec3::new(14.0, 22.0, 0.0))
                .length()
                < 1e-5
        );
        let extent = transform.transform_point3(bounds.max.extend(0.0));
        let aspect = width as f32 / height as f32;
        assert!(extent.x <= aspect * 0.96 + 1e-5);
        assert!(extent.y <= 0.96 + 1e-5);
        assert!((extent.x / aspect - 0.96).abs() < 1e-5 || (extent.y - 0.96).abs() < 1e-5);
        let x = transform.transform_vector3(Vec3::X).length();
        let y = transform.transform_vector3(Vec3::Y).length();
        assert_eq!(x, y, "circles must stay circular");
    }
}

#[test]
fn invalid_bounds_or_viewports_leave_the_default_camera() {
    for bounds in [
        Bounds {
            min: Vec2::ZERO,
            max: Vec2::ZERO,
        },
        Bounds {
            min: Vec2::ONE,
            max: Vec2::ZERO,
        },
        Bounds {
            min: Vec2::ZERO,
            max: Vec2::splat(f32::INFINITY),
        },
        Bounds {
            min: Vec2::splat(f32::NAN),
            max: Vec2::ONE,
        },
    ] {
        assert!(
            Camera::default()
                .transform(Some(bounds), (100, 100))
                .is_none()
        );
    }
    let valid = Bounds {
        min: -Vec2::ONE,
        max: Vec2::ONE,
    };
    assert!(Camera::default().transform(Some(valid), (0, 100)).is_none());
    assert!(Camera::default().transform(Some(valid), (100, 0)).is_none());
}

#[test]
fn zoom_anchors_cursor_in_event_order_and_stays_finite_at_limits() {
    let mut camera = Camera::default();
    let size = (800, 600);
    let point = Vec2::new(620.0, 180.0);
    let anchor = world_at(&camera, bounds(), size, point);
    // The final pointer differs from the scroll location in this same frame.
    camera.update(
        Some(bounds()),
        (800.0, 600.0),
        &input(&[
            Event::Moved(point),
            Event::Scroll(Vec2::new(0.0, 80.0)),
            Event::Moved(Vec2::ZERO),
        ]),
    );
    assert!(world_at(&camera, bounds(), size, point).distance(anchor) < 1e-3);
    let distance = world_at(&camera, bounds(), size, point + Vec2::X).distance(anchor);
    assert!(distance < 1.5, "scroll up must zoom in");

    // Zoom extremes must not yield singular/non-finite rendering transforms.
    for scroll in [f32::MAX, -f32::MAX] {
        camera.update(
            Some(bounds()),
            (800.0, 600.0),
            &input(&[Event::Scroll(Vec2::new(0.0, scroll))]),
        );
        let transform = camera.transform(Some(bounds()), size).unwrap();
        assert!(transform.is_finite());
        assert!(transform.inverse().is_finite());
    }
}

#[test]
fn drag_moves_the_grabbed_world_point_and_cancellation_ends_capture() {
    let mut camera = Camera::default();
    let size = (800, 600);
    let start = Vec2::new(300.0, 250.0);
    let end = Vec2::new(450.0, 350.0);
    let grabbed = world_at(&camera, bounds(), size, start);
    camera.update(
        Some(bounds()),
        (800.0, 600.0),
        &input(&[
            Event::Button {
                button: Button::Primary,
                pressed: true,
                position: start,
            },
            Event::Moved(end),
        ]),
    );
    assert!(world_at(&camera, bounds(), size, end).distance(grabbed) < 1e-3);
    let before = camera.transform(Some(bounds()), size);
    camera.update(
        Some(bounds()),
        (800.0, 600.0),
        &input(&[Event::Cancelled, Event::Moved(start)]),
    );
    assert_eq!(camera.transform(Some(bounds()), size), before);
}

#[test]
fn manual_view_preserves_center_and_magnification_on_resize_until_fit() {
    let mut camera = Camera::default();
    camera.update(
        Some(bounds()),
        (800.0, 600.0),
        &input(&[
            Event::Moved(Vec2::new(500.0, 200.0)),
            Event::Scroll(Vec2::new(0.0, 80.0)),
        ]),
    );
    let center = world_at(&camera, bounds(), (800, 600), Vec2::new(400.0, 300.0));
    let offset = world_at(&camera, bounds(), (800, 600), Vec2::new(420.0, 300.0));
    camera.update(Some(bounds()), (1000.0, 800.0), &input(&[Event::Cancelled]));
    assert!(
        world_at(&camera, bounds(), (1000, 800), Vec2::new(500.0, 400.0)).distance(center) < 1e-3
    );
    assert!(
        world_at(&camera, bounds(), (1000, 800), Vec2::new(520.0, 400.0)).distance(offset) < 1e-3
    );
    // Twice the physical resolution at the same logical size preserves the view.
    assert!(
        world_at(&camera, bounds(), (2000, 1600), Vec2::new(1000.0, 800.0)).distance(center) < 1e-3
    );
    assert!(
        world_at(&camera, bounds(), (2000, 1600), Vec2::new(1040.0, 800.0)).distance(offset) < 1e-3
    );
    camera.fit();
    assert_eq!(
        camera.transform(Some(bounds()), (1000, 800)),
        Camera::default().transform(Some(bounds()), (1000, 800))
    );
}
