use super::*;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn crowding_is_gaussian_and_independent_of_plant_energy() {
    let config = Config {
        density_floor: 0.01,
        density_peak: 0.01,
        crowding_radius: 10.0,
        ..Config::default()
    };
    let patches = PatchConfig::default();
    let sampler = Sampler::new(config, WorldConfig::default(), 10.0, patches).unwrap();
    let landscape = patches::Landscape::default();
    let mut plants = [Plant::new(1, Vec2::ZERO, 0.0)];
    let center = sampler.acceptance(Vec2::ZERO, &plants, &landscape);
    let shoulder = sampler.acceptance(Vec2::new(10.0, 0.0), &plants, &landscape);
    assert!(shoulder > center && shoulder < 1.0);
    assert!(((1.0 - shoulder) / (1.0 - center) - (-0.5_f32).exp()).abs() < 4.0 * f32::EPSILON);
    assert_eq!(
        shoulder,
        sampler.acceptance(Vec2::new(0.0, -10.0), &plants, &landscape)
    );
    plants[0].energy = 1.0;
    assert_eq!(sampler.acceptance(Vec2::ZERO, &plants, &landscape), center);
}

#[test]
fn gaussian_envelope_preserves_the_deficit_in_overlapping_patches() {
    let patches = PatchConfig {
        count: 2,
        spread: 20.0,
        ..PatchConfig::default()
    };
    let landscape = patches::Landscape::generate(
        patches,
        WorldConfig {
            width: 20.0,
            height: 20.0,
        },
        0.0,
        &mut Xoshiro256PlusPlus::seed_from_u64(3),
    )
    .unwrap();
    let config = Config {
        density_floor: 0.002,
        density_peak: 0.01,
        crowding_radius: 10.0,
        ..Config::default()
    };
    let sampler = Sampler::new(config, WorldConfig::default(), 10.0, patches).unwrap();
    assert!(sampler.gaussian_envelope);
    let fertility = landscape.fertility(Vec2::ZERO, patches);
    let capacity = 0.002 + 0.008 * fertility;
    let envelope = sampler.envelope(Vec2::ZERO, &landscape);
    assert!(envelope > capacity);
    let plants = [Plant::new(1, Vec2::ZERO, 0.0)];
    let deficit = capacity - sampler.kernel_scale(Vec2::ZERO);
    let acceptance = sampler.acceptance(Vec2::ZERO, &plants, &landscape);
    assert!(acceptance > 0.0 && acceptance < 1.0);
    assert!((envelope * acceptance - deficit).abs() < 4.0 * f32::EPSILON * capacity);
}

#[test]
fn gaussian_proposals_have_the_expected_moments_and_spend_outside_attempts() {
    let config = Config {
        density_floor: 0.0,
        ..Config::default()
    };
    let patches = PatchConfig {
        count: 1,
        spread: 5.0,
        ..PatchConfig::default()
    };
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(29);
    let landscape = patches::Landscape::generate(
        patches,
        WorldConfig {
            width: 2.0,
            height: 2.0,
        },
        0.0,
        &mut rng,
    )
    .unwrap();
    let sampler = Sampler::new(config, WorldConfig::default(), 10.0, patches).unwrap();
    assert!(sampler.gaussian_envelope);
    let center = landscape.centers()[0];
    let mut sum = Vec2::ZERO;
    let mut squares = Vec2::ZERO;
    for _ in 0..4096 {
        let offset = sampler.propose(&[], &landscape, &mut rng).unwrap() - center;
        sum += offset;
        squares += offset * offset;
    }
    assert!((sum / 4096.0).abs().max_element() < 0.3);
    assert!((squares / (4096.0 * 25.0) - Vec2::ONE).abs().max_element() < 0.1);

    let thin = Sampler::new(
        config,
        WorldConfig {
            width: 2.0,
            height: 100.0,
        },
        0.0,
        PatchConfig {
            spread: 1.0,
            ..patches
        },
    )
    .unwrap();
    assert!(thin.gaussian_envelope);
    let mut rejected = 0;
    for _ in 0..4096 {
        match thin.propose(&[], &landscape, &mut rng) {
            Some(position) => {
                assert!(position.is_finite() && position.abs().cmple(thin.limit).all())
            }
            None => rejected += 1,
        }
    }
    assert!((1000..2500).contains(&rejected));
}

#[test]
fn accepted_seedlings_immediately_suppress_further_proposals() {
    let sampler = Sampler::new(
        Config {
            density_floor: 0.1,
            density_peak: 0.1,
            crowding_radius: 1000.0,
            ..Config::default()
        },
        WorldConfig {
            width: 2.0,
            height: 2.0,
        },
        0.0,
        PatchConfig::default(),
    )
    .unwrap();
    let landscape = patches::Landscape::default();
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(11);
    let position = sampler.propose(&[], &landscape, &mut rng).unwrap();
    let plants = [Plant::new(1, position, 0.0)];
    for _ in 0..16 {
        assert!(sampler.propose(&plants, &landscape, &mut rng).is_none());
    }
}

#[test]
fn boundary_correction_restores_missing_kernel_mass() {
    let sampler = Sampler::new(
        Config {
            crowding_radius: 10.0,
            ..Config::default()
        },
        WorldConfig::default(),
        10.0,
        PatchConfig::default(),
    )
    .unwrap();
    assert!(
        (sampler.kernel_scale(sampler.limit) / sampler.kernel_scale(Vec2::ZERO) - 4.0).abs()
            < 8.0 * f32::EPSILON
    );

    let broad = Sampler::new(
        Config {
            crowding_radius: 1e9,
            ..Config::default()
        },
        WorldConfig {
            width: 120.0,
            height: 80.0,
        },
        10.0,
        PatchConfig::default(),
    )
    .unwrap();
    for position in [Vec2::ZERO, broad.limit] {
        assert!((broad.kernel_scale(position) * 6000.0 - 1.0).abs() < 8.0 * f32::EPSILON);
    }
}

#[test]
fn proposal_budgets_use_bounded_stochastic_rounding() {
    let world = WorldConfig {
        width: 10.0,
        height: 10.0,
    };
    let base = Config {
        density_floor: 0.01,
        density_peak: 0.01,
        max_attempts_per_tick: 3,
        ..Config::default()
    };
    for (rate, lower, upper) in [(0.25, 0, 1), (2.75, 2, 3)] {
        let config = Config {
            recovery_rate: rate,
            ..base
        };
        config.validate(world, 0.0, PatchConfig::default()).unwrap();
        let sampler = Sampler::new(config, world, 0.0, PatchConfig::default()).unwrap();
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(19);
        let mut total = 0;
        for _ in 0..2048 {
            let attempts = sampler.attempts(&mut rng);
            assert!((lower..=upper).contains(&attempts));
            total += attempts;
        }
        assert!((total as f32 / 2048.0 - rate).abs() < 0.05);
    }
    let zero = Sampler::new(
        Config {
            recovery_rate: 0.0,
            ..base
        },
        world,
        0.0,
        PatchConfig::default(),
    )
    .unwrap();
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(19);
    let unchanged = rng.clone();
    assert_eq!(zero.attempts(&mut rng), 0);
    assert_eq!(rng, unchanged);
}

#[test]
fn invalid_density_and_proposal_settings_are_rejected() {
    let world = WorldConfig::default();
    let patches = PatchConfig::default();
    let invalid_changes: &[fn(&mut Config)] = &[
        |c| c.density_floor = -1.0,
        |c| c.density_floor = 1.0,
        |c| c.density_peak = 0.0,
        |c| c.density_peak = f32::INFINITY,
        |c| c.crowding_radius = 0.0,
        |c| c.crowding_radius = f32::NAN,
        |c| c.crowding_radius = f32::MIN_POSITIVE,
        |c| c.recovery_rate = -1.0,
        |c| c.recovery_rate = f32::NAN,
        |c| c.recovery_rate = f32::MAX,
        |c| c.max_attempts_per_tick = 5,
    ];
    for change in invalid_changes {
        let mut config = Config::default();
        change(&mut config);
        assert!(config.validate(world, 10.0, patches).is_err());
    }
    let varying = Config {
        density_floor: 0.0,
        ..Config::default()
    };
    assert!(varying.validate(world, 10.0, patches).is_err());
    varying
        .validate(
            world,
            10.0,
            PatchConfig {
                count: 1,
                ..patches
            },
        )
        .unwrap();
    for spread in [1000.0, f32::MAX, f32::MIN_POSITIVE] {
        let sampler = Sampler::new(
            varying,
            world,
            10.0,
            PatchConfig {
                count: 1,
                spread,
                ..patches
            },
        )
        .unwrap();
        assert!(!sampler.gaussian_envelope);
        assert!((sampler.proposal_mass - 505.92).abs() < 2.0 * f32::EPSILON * 505.92);
    }
    assert!(
        Config {
            density_peak: f32::MAX,
            ..varying
        }
        .validate(
            world,
            10.0,
            PatchConfig {
                count: 1,
                ..patches
            }
        )
        .is_err()
    );
    assert!(Config::default().validate(world, 350.0, patches).is_err());
    assert!(
        Config::default()
            .validate(
                WorldConfig {
                    width: f32::MAX,
                    height: f32::MAX
                },
                0.0,
                patches,
            )
            .is_err()
    );
}
