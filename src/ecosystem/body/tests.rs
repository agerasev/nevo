use super::*;

#[test]
fn disk_mass_and_inertia_follow_stored_energy() {
    let mut body = Body::new(DVec2::ZERO, 4.0);
    assert_eq!(body.radius(), 20.0);
    assert_eq!(body.mass(3.0), 12.0);
    assert_eq!(body.inertia(3.0), 2400.0);
    for energy in [0.0, -1.0] {
        body.energy = energy;
        assert_eq!(body.radius(), 0.0);
        assert_eq!(body.mass(3.0), 0.0);
        assert_eq!(body.inertia(3.0), 0.0);
    }
}

#[test]
fn body_coordinates_are_forward_and_left() {
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.heading = PI / 2.0;
    assert!(body.forward().distance(DVec2::Y) < 1e-15);
    assert!(
        body.world_to_body(DVec2::new(3.0, 4.0))
            .distance(DVec2::new(4.0, -3.0))
            < 1e-14
    );
}

#[test]
fn confinement_removes_only_outward_normal_velocity() {
    let world = WorldConfig {
        width: 100.0,
        height: 80.0,
    };
    let mut body = Body::new(DVec2::new(60.0, 0.0), 1.0);
    body.velocity = DVec2::new(4.0, 3.0);
    body.confine(world);
    assert_eq!(body.position, DVec2::new(40.0, 0.0));
    assert_eq!(body.velocity, DVec2::new(0.0, 3.0));
    body.velocity.x = -2.0;
    body.confine(world);
    assert_eq!(body.velocity, DVec2::new(-2.0, 3.0));
}

#[test]
fn invalid_kinematics_are_rejected() {
    let edits: [fn(&mut Body); 5] = [
        |body| body.heading = f64::NAN,
        |body| body.heading = PI,
        |body| body.heading = -PI - 0.1,
        |body| body.velocity.x = f64::INFINITY,
        |body| body.angular_velocity = f64::NAN,
    ];
    for edit in edits {
        let mut body = Body::new(DVec2::ZERO, 1.0);
        edit(&mut body);
        assert!(
            body.validate(WorldConfig::default(), BodyParameters::default())
                .is_err()
        );
    }
}
