use glam::{Vec2, Vec3};

use super::Bounds;

#[test]
fn camera_centers_offset_bounds_and_preserves_aspect_with_padding() {
    let bounds = Bounds {
        min: Vec2::new(10.0, 20.0),
        max: Vec2::new(18.0, 24.0),
    };
    for (width, height) in [(400, 200), (200, 400), (200, 200)] {
        let transform = bounds.transform((width, height)).unwrap();
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
        assert!(bounds.transform((100, 100)).is_none());
    }
    let valid = Bounds {
        min: -Vec2::ONE,
        max: Vec2::ONE,
    };
    assert!(valid.transform((0, 100)).is_none());
    assert!(valid.transform((100, 0)).is_none());
}
