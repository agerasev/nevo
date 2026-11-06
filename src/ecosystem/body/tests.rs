use super::*;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn passive_motion_damps_without_motor_work() -> Result<()> {
    let parameters = BodyParameters {
        time_step: 0.5,
        ..BodyParameters::default()
    };
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.velocity = DVec2::new(3.0, -4.0);
    body.angular_velocity = 2.0;
    let motion = body.advance(MotorAction::default(), parameters, WorldConfig::default())?;
    close(
        body.velocity.length(),
        5.0 * (-parameters.linear_drag * parameters.time_step).exp(),
    );
    close(
        body.angular_velocity,
        2.0 * (-parameters.angular_drag * parameters.time_step).exp(),
    );
    close(motion.mechanical_work, 0.0);
    close(motion.force_squared_time, 0.0);
    close(motion.torque_squared_time, 0.0);
    assert!(motion.distance > 0.0);
    close(body.energy, 1.0);
    Ok(())
}

#[test]
fn forward_work_without_drag_equals_kinetic_energy_gain() -> Result<()> {
    let parameters = BodyParameters {
        mass: 2.0,
        linear_drag: 0.0,
        ..BodyParameters::default()
    };
    let mut body = Body::new(DVec2::ZERO, 1.0);
    let motion = body.advance(
        MotorAction {
            thrust: 0.6,
            turn: 0.0,
        },
        parameters,
        WorldConfig::default(),
    )?;
    close(
        motion.mechanical_work,
        0.5 * parameters.mass * body.velocity.length_squared(),
    );
    close(
        motion.distance,
        0.5 * 0.6 * parameters.max_acceleration * parameters.time_step.powi(2),
    );
    close(
        motion.force_squared_time,
        (parameters.mass * 0.6 * parameters.max_acceleration).powi(2) * parameters.time_step,
    );
    close(body.velocity.y, 0.0);
    Ok(())
}

#[test]
fn pure_turn_without_drag_costs_rotational_work() -> Result<()> {
    let parameters = BodyParameters {
        moment_of_inertia: 3.0,
        angular_drag: 0.0,
        ..BodyParameters::default()
    };
    let mut body = Body::new(DVec2::ZERO, 1.0);
    let motion = body.advance(
        MotorAction {
            thrust: 0.0,
            turn: -0.8,
        },
        parameters,
        WorldConfig::default(),
    )?;
    close(
        motion.mechanical_work,
        0.5 * parameters.moment_of_inertia * body.angular_velocity.powi(2),
    );
    close(
        motion.torque_squared_time,
        (parameters.moment_of_inertia * -0.8 * parameters.max_angular_acceleration).powi(2)
            * parameters.time_step,
    );
    close(motion.distance, 0.0);
    assert!(body.heading < 0.0);
    Ok(())
}

#[test]
fn rotating_the_scene_rotates_motion_and_preserves_work() -> Result<()> {
    let parameters = BodyParameters {
        time_step: 0.4,
        ..BodyParameters::default()
    };
    let mut original = Body::new(DVec2::new(5.0, -2.0), 1.0);
    original.heading = 0.2;
    original.velocity = DVec2::new(2.0, 3.0);
    original.angular_velocity = 0.6;
    let rotation = direction(1.2);
    let mut rotated = original.clone();
    rotated.position = rotation.rotate(original.position);
    rotated.velocity = rotation.rotate(original.velocity);
    rotated.heading += 1.2;
    let action = MotorAction {
        thrust: 0.4,
        turn: -0.5,
    };
    let motion = original.advance(action, parameters, WorldConfig::default())?;
    let rotated_motion = rotated.advance(action, parameters, WorldConfig::default())?;
    close(
        rotated
            .position
            .distance(rotation.rotate(original.position)),
        0.0,
    );
    close(
        rotated
            .velocity
            .distance(rotation.rotate(original.velocity)),
        0.0,
    );
    close(rotated.heading, original.heading + 1.2);
    close(rotated_motion.mechanical_work, motion.mechanical_work);
    close(rotated_motion.distance, motion.distance);
    close(
        original
            .world_to_body(original.velocity)
            .distance(rotated.world_to_body(rotated.velocity)),
        0.0,
    );
    Ok(())
}

#[test]
fn wall_contact_removes_outward_velocity_but_retains_tangent() -> Result<()> {
    let parameters = BodyParameters {
        linear_drag: 0.0,
        ..BodyParameters::default()
    };
    let world = WorldConfig {
        width: 100.0,
        height: 100.0,
    };
    let mut body = Body::new(DVec2::new(39.0, 0.0), 1.0);
    body.velocity = DVec2::new(50.0, 3.0);
    body.advance(MotorAction::default(), parameters, world)?;
    close(body.position.x, world.half_size().x - body.radius());
    close(body.velocity.x, 0.0);
    close(body.velocity.y, 3.0);
    close(body.position.y, 0.3);
    let motion = body.advance(
        MotorAction {
            thrust: 1.0,
            turn: 0.0,
        },
        parameters,
        world,
    )?;
    close(motion.mechanical_work, 0.0);
    assert!(motion.force_squared_time > 0.0);
    close(body.velocity.x, 0.0);
    body.velocity.x = -2.0;
    body.confine(world);
    close(body.velocity.x, -2.0);
    Ok(())
}

#[test]
fn turning_trajectory_converges_with_more_substeps() -> Result<()> {
    let action = MotorAction {
        thrust: 0.7,
        turn: 0.6,
    };
    let simulate = |substeps| -> Result<Body> {
        let parameters = BodyParameters {
            time_step: 1.0,
            substeps,
            ..BodyParameters::default()
        };
        let mut body = Body::new(DVec2::ZERO, 1.0);
        body.advance(action, parameters, WorldConfig::default())?;
        Ok(body)
    };
    let coarse = simulate(4)?;
    let fine = simulate(16)?;
    let reference = simulate(256)?;
    assert!(
        fine.position.distance(reference.position)
            < coarse.position.distance(reference.position) / 8.0
    );
    assert!(
        fine.velocity.distance(reference.velocity)
            < coarse.velocity.distance(reference.velocity) / 8.0
    );
    close(coarse.heading, reference.heading);
    close(coarse.angular_velocity, reference.angular_velocity);
    Ok(())
}

#[test]
fn tiny_drag_matches_zero_drag_without_cancellation() -> Result<()> {
    let action = MotorAction {
        thrust: 0.7,
        turn: 0.4,
    };
    let mut zero = Body::new(DVec2::ZERO, 1.0);
    let mut tiny = zero.clone();
    let parameters = BodyParameters {
        linear_drag: 0.0,
        angular_drag: 0.0,
        ..BodyParameters::default()
    };
    zero.advance(action, parameters, WorldConfig::default())?;
    tiny.advance(
        action,
        BodyParameters {
            linear_drag: 1e-14,
            angular_drag: 1e-14,
            ..parameters
        },
        WorldConfig::default(),
    )?;
    close(zero.position.distance(tiny.position), 0.0);
    close(zero.velocity.distance(tiny.velocity), 0.0);
    close(zero.heading, tiny.heading);
    Ok(())
}

#[test]
fn forward_reversal_charges_positive_work_without_crediting_braking() -> Result<()> {
    for (drag, time_step, acceleration, expected_work) in [
        (0.0, 1.0, 2.0, 0.5),
        (1.0, 4.0_f64.ln(), 1.0, 2.0_f64.ln() - 0.5),
    ] {
        let parameters = BodyParameters {
            linear_drag: drag,
            max_acceleration: acceleration,
            time_step,
            substeps: 1,
            ..BodyParameters::default()
        };
        let mut body = Body::new(DVec2::ZERO, 1.0);
        body.velocity = DVec2::NEG_X;
        let motion = body.advance(
            MotorAction {
                thrust: 1.0,
                turn: 0.0,
            },
            parameters,
            WorldConfig::default(),
        )?;
        close(motion.mechanical_work, expected_work);
        assert!(body.velocity.x > 0.0);
    }
    Ok(())
}

#[test]
fn angular_reversal_charges_work_even_with_zero_net_rotation() -> Result<()> {
    let parameters = BodyParameters {
        moment_of_inertia: 3.0,
        angular_drag: 0.0,
        max_angular_acceleration: 2.0,
        time_step: 1.0,
        substeps: 1,
        ..BodyParameters::default()
    };
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.angular_velocity = 1.0;
    let motion = body.advance(
        MotorAction {
            thrust: 0.0,
            turn: -1.0,
        },
        parameters,
        WorldConfig::default(),
    )?;
    close(body.heading, 0.0);
    close(body.angular_velocity, -1.0);
    close(motion.mechanical_work, 1.5);
    Ok(())
}

#[test]
fn forward_reversal_stops_charging_work_at_the_wall() -> Result<()> {
    let parameters = BodyParameters {
        linear_drag: 0.0,
        max_acceleration: 2.0,
        time_step: 1.5,
        substeps: 1,
        ..BodyParameters::default()
    };
    let world = WorldConfig {
        width: 100.0,
        height: 100.0,
    };
    let mut body = Body::new(DVec2::new(39.5, 0.0), 1.0);
    body.velocity = DVec2::NEG_X;
    let motion = body.advance(
        MotorAction {
            thrust: 1.0,
            turn: 0.0,
        },
        parameters,
        world,
    )?;
    // Reverse at x=39.25, then apply force 2 until the center reaches x=40.
    close(motion.mechanical_work, 1.5);
    close(body.position.x, 40.0);
    close(body.velocity.x, 0.0);
    Ok(())
}
