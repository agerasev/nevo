use super::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn gaussian_fertility_is_symmetric_and_overlaps_stay_bounded() {
    let config = Config {
        count: 1,
        spread: 10.0,
        growth_floor: 0.0,
        ..Config::default()
    };
    let single = Landscape {
        centers: vec![DVec2::ZERO],
    };
    assert_eq!(single.growth_multiplier(DVec2::ZERO, config), 1.0);
    let shoulder = single.growth_multiplier(DVec2::new(10.0, 0.0), config);
    assert!((shoulder - 0.6065306597126334).abs() < 1e-12);
    assert_eq!(
        shoulder,
        single.growth_multiplier(DVec2::new(0.0, -10.0), config)
    );
    let overlapping = Landscape {
        centers: vec![DVec2::new(-10.0, 0.0), DVec2::new(10.0, 0.0)],
    };
    let overlap = overlapping.growth_multiplier(DVec2::ZERO, Config { count: 2, ..config });
    assert!(overlap > shoulder && overlap < 1.0);
    assert_eq!(single.growth_multiplier(DVec2::splat(1000.0), config), 0.0);
}

#[test]
fn density_does_not_change_growth_and_an_empty_landscape_has_constant_growth() {
    let landscape = Landscape {
        centers: vec![DVec2::ZERO],
    };
    let mut config = Config {
        count: 1,
        growth_floor: 0.5,
        growth_peak: 2.0,
        ..Config::default()
    };
    let growth = landscape.growth_multiplier(DVec2::new(100.0, 0.0), config);
    config.density_fraction = 1.0;
    assert_eq!(
        landscape.growth_multiplier(DVec2::new(100.0, 0.0), config),
        growth
    );
    let uniform = Config {
        growth_floor: 0.25,
        growth_peak: 0.25,
        ..Config::default()
    };
    assert_eq!(
        Landscape::default().growth_multiplier(DVec2::ZERO, uniform),
        0.25
    );
}

#[test]
fn truncated_normal_sampling_is_bounded_and_remains_clustered() {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(8);
    let mut mean = 0.0;
    let mut second_moment = 0.0;
    for _ in 0..2048 {
        let value = sample_axis(0.0, 1.0, 100.0, &mut rng).unwrap();
        assert!(value.abs() <= 100.0);
        mean += value;
        second_moment += value * value;
    }
    assert!((mean / 2048.0).abs() < 0.1);
    assert!((0.85..1.15).contains(&(second_moment / 2048.0)));
    for spread in [1.0, 200.0, 1000.0] {
        for _ in 0..256 {
            let value = sample_axis(100.0, spread, 100.0, &mut rng).unwrap();
            assert!((-100.0..=100.0).contains(&value));
        }
    }
}

#[test]
fn disabled_patches_preserve_uniform_sampling_and_rng_consumption() {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(17);
    let mut expected_rng = rng.clone();
    let landscape =
        Landscape::generate(Config::default(), WorldConfig::default(), 10.0, &mut rng).unwrap();
    let position = landscape
        .position(Config::default(), WorldConfig::default(), 10.0, &mut rng)
        .unwrap();
    assert_eq!(
        position,
        random_position(&mut expected_rng, WorldConfig::default())
    );
    assert_eq!(rng, expected_rng);
}

#[test]
fn invalid_patch_settings_and_stored_centers_are_rejected() {
    let world = WorldConfig::default();
    let invalid_changes: &[fn(&mut Config)] = &[
        |c| c.spread = 0.0,
        |c| c.spread = f64::NAN,
        |c| c.density_fraction = -0.1,
        |c| c.density_fraction = 1.1,
        |c| c.density_fraction = 0.5,
        |c| c.growth_floor = -1.0,
        |c| c.growth_peak = f64::INFINITY,
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
        growth_floor: f64::MAX,
        growth_peak: f64::MAX,
        ..Config::default()
    };
    assert!(overflow.validate(world, 10.0, 2.0).is_err());
    for centers in [
        vec![],
        vec![DVec2::splat(f64::NAN)],
        vec![DVec2::new(625.0, 0.0)],
    ] {
        assert!(Landscape { centers }.validate(config, world, 10.0).is_err());
    }
}
