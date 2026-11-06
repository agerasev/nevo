use super::*;

#[test]
fn obstacles_may_touch_walls_and_overlap_each_other() {
    let world = WorldConfig {
        width: 10.0,
        height: 8.0,
    };
    let obstacles = [
        Obstacle {
            position: Vec2::new(4.0, 0.0),
            radius: 1.0,
        },
        Obstacle {
            position: Vec2::new(3.0, 0.0),
            radius: 2.0,
        },
    ];
    validate(&obstacles, world).unwrap();
}

#[test]
fn invalid_shapes_and_circles_crossing_the_arena_are_rejected() {
    let world = WorldConfig {
        width: 10.0,
        height: 8.0,
    };
    for (position, radius) in [
        (Vec2::ZERO, 0.0),
        (Vec2::ZERO, -1.0),
        (Vec2::ZERO, f32::NAN),
        (Vec2::ZERO, f32::INFINITY),
        (Vec2::splat(f32::NAN), 1.0),
        (Vec2::splat(f32::INFINITY), 1.0),
        (Vec2::new(4.5, 0.0), 1.0),
        (Vec2::ZERO, 5.0),
    ] {
        assert!(validate(&[Obstacle { position, radius }], world).is_err());
    }
}

#[test]
fn clearance_checks_every_circle_and_allows_tangency() {
    let obstacles = [
        Obstacle {
            position: Vec2::ZERO,
            radius: 1.0,
        },
        Obstacle {
            position: Vec2::new(3.0, 0.0),
            radius: 1.0,
        },
    ];
    assert!(clear(Vec2::new(-2.0, 0.0), 1.0, &obstacles));
    assert!(!clear(Vec2::new(-1.9, 0.0), 1.0, &obstacles));
    assert!(clear(Vec2::new(3.0, 2.0), 1.0, &obstacles));
    assert!(!clear(Vec2::new(3.0, 1.9), 1.0, &obstacles));
    assert!(clear(Vec2::new(-1.0, 0.0), 0.0, &obstacles));
    assert!(!clear(Vec2::ZERO, 0.0, &obstacles));
    assert!(clear(Vec2::ZERO, 1.0, &[]));
}
