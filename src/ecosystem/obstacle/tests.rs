use super::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn random_circles_fit_the_arena_and_configured_radius_range() {
    let world = WorldConfig::default();
    let config = Config::Random(RandomConfig {
        count: 32,
        radius_min: 12.0,
        radius_max: 24.0,
    });
    let mut obstacles = config
        .generate(world, &mut Xoshiro256PlusPlus::seed_from_u64(1))
        .unwrap();
    assert_eq!(obstacles.len(), 32);
    config.validate_layout(&obstacles, world).unwrap();
    assert!(
        obstacles
            .iter()
            .any(|obstacle| obstacle.radius != obstacles[0].radius)
    );
    obstacles.pop();
    assert!(config.validate_layout(&obstacles, world).is_err());
}

#[test]
fn invalid_random_layout_settings_are_rejected() {
    for (count, radius_min, radius_max) in [
        (1, 0.0, 24.0),
        (1, 24.0, 12.0),
        (1, f32::NAN, 24.0),
        (1, 12.0, f32::INFINITY),
        (1, 12.0, 400.0),
        (usize::MAX, 12.0, 24.0),
    ] {
        let config = Config::Random(RandomConfig {
            count,
            radius_min,
            radius_max,
        });
        assert!(config.validate(WorldConfig::default()).is_err());
    }
}

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
