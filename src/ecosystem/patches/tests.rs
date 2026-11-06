use super::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn gaussian_fertility_is_symmetric_and_overlaps_stay_bounded() {
    let config = Config {
        count: 1,
        spread: 10.0,
        ..Config::default()
    };
    let single = Landscape {
        centers: vec![Vec2::ZERO],
    };
    assert_eq!(single.fertility(Vec2::ZERO, config), 1.0);
    let shoulder = single.fertility(Vec2::new(10.0, 0.0), config);
    assert!((shoulder - (-0.5_f32).exp()).abs() < f32::EPSILON);
    assert_eq!(shoulder, single.fertility(Vec2::new(0.0, -10.0), config));
    let overlapping = Landscape {
        centers: vec![Vec2::new(-10.0, 0.0), Vec2::new(10.0, 0.0)],
    };
    let overlap = overlapping.fertility(Vec2::ZERO, Config { count: 2, ..config });
    assert!(overlap > shoulder && overlap < 1.0);
    assert_eq!(single.fertility(Vec2::splat(1000.0), config), 0.0);
}

#[test]
fn growth_multipliers_scale_fertility_and_empty_landscapes_are_constant() {
    let landscape = Landscape {
        centers: vec![Vec2::ZERO],
    };
    let config = Config {
        count: 1,
        growth_floor: 0.5,
        growth_peak: 2.0,
        ..Config::default()
    };
    assert_eq!(landscape.growth_multiplier(Vec2::ZERO, config), 2.0);
    assert_eq!(
        landscape.growth_multiplier(Vec2::splat(10000.0), config),
        0.5
    );
    let uniform = Config {
        growth_floor: 0.25,
        growth_peak: 0.25,
        ..Config::default()
    };
    assert_eq!(
        Landscape::default().growth_multiplier(Vec2::ZERO, uniform),
        0.25
    );
    assert_eq!(Landscape::default().fertility(Vec2::ZERO, uniform), 0.0);
}

#[test]
fn generating_no_patches_consumes_no_randomness() {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(17);
    let expected_rng = rng.clone();
    let landscape =
        Landscape::generate(Config::default(), WorldConfig::default(), 10.0, &mut rng).unwrap();
    assert_eq!(landscape, Landscape::default());
    assert_eq!(rng, expected_rng);
}

#[test]
fn invalid_patch_settings_and_stored_centers_are_rejected() {
    let world = WorldConfig::default();
    let invalid_changes: &[fn(&mut Config)] = &[
        |c| c.spread = 0.0,
        |c| c.spread = f32::NAN,
        |c| c.growth_floor = -1.0,
        |c| c.growth_peak = f32::INFINITY,
        |c| c.growth_peak = 0.5,
        |c| c.growth_peak = 2.0,
        |c| c.count = usize::MAX,
    ];
    for change in invalid_changes {
        let mut config = Config::default();
        change(&mut config);
        assert!(config.validate(world, 10.0, 0.0025).is_err());
    }
    let config = Config {
        count: 1,
        ..Config::default()
    };
    assert!(config.validate(world, 350.0, 0.0025).is_err());
    let overflow = Config {
        growth_floor: f32::MAX,
        growth_peak: f32::MAX,
        ..Config::default()
    };
    assert!(overflow.validate(world, 10.0, 2.0).is_err());
    for centers in [
        vec![],
        vec![Vec2::splat(f32::NAN)],
        vec![Vec2::new(625.0, 0.0)],
    ] {
        assert!(Landscape { centers }.validate(config, world, 10.0).is_err());
    }
}
