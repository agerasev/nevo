use super::*;
use crate::ecosystem::Config;

fn parameters() -> Parameters {
    let mut parameters = <Config>::default().parameters();
    parameters.body.linear_drag = 0.0;
    parameters.body.angular_drag = 0.0;
    parameters.plant.linear_drag = 0.0;
    parameters.plant.angular_drag = 0.0;
    parameters
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{actual} != {expected}"
    );
}

#[test]
fn unequal_mass_contact_conserves_pair_momentum() -> Result<()> {
    let parameters = parameters();
    let mut body = Body::new(DVec2::new(-12.5, 0.0), 4.0);
    let mut plants = [Plant::new(1, DVec2::new(12.5, 0.0), 1.0)];
    let step = advance(
        &mut [(&mut body, MotorAction::default())],
        &mut plants,
        &[],
        parameters,
    )?;
    assert!(body.velocity.x < 0.0 && plants[0].velocity.x > 0.0);
    assert!(body.position.distance(plants[0].position) > 25.0);
    assert!((4.0 * body.velocity + plants[0].velocity).length() < 1e-12);
    assert_eq!(step.contacts, BTreeSet::from([(0, 0)]));
    assert_eq!(step.motions[0].mechanical_work, 0.0);
    Ok(())
}

#[test]
fn plants_repel_without_motors() -> Result<()> {
    let mut plants = [
        Plant::new(1, DVec2::new(-9.0, 0.0), 1.0),
        Plant::new(2, DVec2::new(9.0, 0.0), 1.0),
    ];
    advance(&mut [], &mut plants, &[], parameters())?;
    assert!(plants[0].position.x < -9.0 && plants[1].position.x > 9.0);
    assert!((plants[0].velocity + plants[1].velocity).length() < 1e-12);
    Ok(())
}

#[test]
fn obstacle_contact_damps_an_incoming_disk() -> Result<()> {
    let mut body = Body::new(DVec2::new(-21.0, 0.0), 1.0);
    body.velocity = DVec2::new(50.0, 0.0);
    let obstacles = [Obstacle {
        position: DVec2::ZERO,
        radius: 10.0,
    }];
    let step = advance(
        &mut [(&mut body, MotorAction::default())],
        &mut [],
        &obstacles,
        parameters(),
    )?;
    assert!(body.position.x < 0.0);
    assert!(body.velocity.length() < 50.0);
    assert_eq!(obstacles[0].position, DVec2::ZERO);
    assert_eq!(step.motions[0].mechanical_work, 0.0);
    Ok(())
}

#[test]
fn fixed_steps_detect_contact_during_fast_travel() -> Result<()> {
    let mut parameters = parameters();
    parameters.physics.substeps = 16;
    let mut body = Body::new(DVec2::new(-40.0, 0.0), 1.0);
    body.velocity = DVec2::new(1000.0, 0.0);
    let mut plants = [Plant::new(1, DVec2::ZERO, 1.0)];
    let step = advance(
        &mut [(&mut body, MotorAction::default())],
        &mut plants,
        &[],
        parameters,
    )?;
    assert!(step.contacts.contains(&(0, 0)));
    assert!(plants[0].velocity.x > 0.0);
    Ok(())
}

#[test]
fn zero_energy_seedlings_stay_inert_and_do_not_contact() -> Result<()> {
    let mut body = Body::new(DVec2::ZERO, 1.0);
    let mut plants = [Plant::new(1, DVec2::ZERO, 0.0)];
    plants[0].velocity = DVec2::new(100.0, -50.0);
    plants[0].angular_velocity = 3.0;
    let step = advance(
        &mut [(&mut body, MotorAction::default())],
        &mut plants,
        &[],
        parameters(),
    )?;
    assert_eq!(plants[0].position, DVec2::ZERO);
    assert_eq!(plants[0].velocity, DVec2::ZERO);
    assert_eq!(plants[0].angular_velocity, 0.0);
    assert_eq!(body.velocity, DVec2::ZERO);
    assert!(step.contacts.is_empty());
    Ok(())
}

#[test]
fn forward_reversal_charges_positive_work_without_braking_credit() -> Result<()> {
    let mut parameters = parameters();
    parameters.physics.time_step = 1.0;
    parameters.body.max_acceleration = 2.0;
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.velocity = DVec2::NEG_X;
    let step = advance(
        &mut [(
            &mut body,
            MotorAction {
                thrust: 1.0,
                turn: 0.0,
            },
        )],
        &mut [],
        &[],
        parameters,
    )?;
    close(body.velocity.x, 1.0, 1e-12);
    close(body.position.x, 0.0, 1e-12);
    close(step.motions[0].mechanical_work, 0.5, 1e-6);
    close(step.motions[0].force_squared_time, 4.0, 1e-12);
    Ok(())
}

#[test]
fn angular_reversal_charges_work_despite_zero_net_rotation() -> Result<()> {
    let mut parameters = parameters();
    parameters.physics.time_step = 1.0;
    parameters.body.max_angular_acceleration = 2.0;
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.angular_velocity = 1.0;
    let step = advance(
        &mut [(
            &mut body,
            MotorAction {
                thrust: 0.0,
                turn: -1.0,
            },
        )],
        &mut [],
        &[],
        parameters,
    )?;
    close(body.heading, 0.0, 1e-12);
    close(body.angular_velocity, -1.0, 1e-12);
    close(step.motions[0].mechanical_work, 25.0, 1e-5);
    close(step.motions[0].torque_squared_time, 10000.0, 1e-8);
    Ok(())
}

#[test]
fn passive_motion_damps_without_motor_costs() -> Result<()> {
    let parameters = <Config>::default().parameters();
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.velocity = DVec2::new(3.0, -4.0);
    body.angular_velocity = 2.0;
    let mut plants = [Plant::new(1, DVec2::new(100.0, 0.0), 1.0)];
    plants[0].velocity = body.velocity;
    plants[0].angular_velocity = body.angular_velocity;
    let step = advance(
        &mut [(&mut body, MotorAction::default())],
        &mut plants,
        &[],
        parameters,
    )?;
    close(
        body.velocity.length(),
        5.0 * (-parameters.physics.time_step).exp(),
        1e-8,
    );
    close(
        body.angular_velocity,
        2.0 * (-2.0 * parameters.physics.time_step).exp(),
        3e-8,
    );
    assert_eq!(body.velocity, plants[0].velocity);
    assert_eq!(body.angular_velocity, plants[0].angular_velocity);
    let motion = step.motions[0];
    assert!(motion.distance > 0.0);
    assert_eq!(motion.mechanical_work, 0.0);
    assert_eq!(motion.force_squared_time, 0.0);
    assert_eq!(motion.torque_squared_time, 0.0);
    Ok(())
}

#[test]
fn numerical_failure_leaves_all_input_state_unchanged() {
    let mut parameters = parameters();
    // Finite motor force whose squared loss overflows during integration.
    parameters.body.max_acceleration = 1e200;
    let mut body = Body::new(DVec2::ZERO, 1.0);
    body.velocity = DVec2::X;
    let mut plants = [Plant::new(1, DVec2::new(100.0, 0.0), 1.0)];
    plants[0].velocity = DVec2::Y;
    let before = (body.clone(), plants.clone());
    let error = advance(
        &mut [(
            &mut body,
            MotorAction {
                thrust: 1.0,
                turn: 0.0,
            },
        )],
        &mut plants,
        &[],
        parameters,
    )
    .err()
    .expect("overflow must fail without committing motion");
    assert!(error.to_string().contains("non-finite physical derivative"));
    assert_eq!((body, plants), before);
}

#[test]
fn contact_motion_agrees_with_a_four_times_smaller_step() -> Result<()> {
    for target in ["plant", "obstacle", "wall"] {
        let integrate = |substeps| -> Result<(Body, Vec<Plant>)> {
            let mut parameters = parameters();
            parameters.physics.substeps = substeps;
            let mut body = Body::new(DVec2::new(-20.0, 0.0), 1.0);
            body.velocity = DVec2::new(50.0, 0.0);
            let mut plants = Vec::new();
            let mut obstacles = Vec::new();
            match target {
                "plant" => plants.push(Plant::new(1, DVec2::ZERO, 1.0)),
                "obstacle" => obstacles.push(Obstacle {
                    position: DVec2::ZERO,
                    radius: 10.0,
                }),
                "wall" => body.position.x = parameters.world.half_size().x - body.radius(),
                _ => unreachable!(),
            }
            for _ in 0..20 {
                advance(
                    &mut [(&mut body, MotorAction::default())],
                    &mut plants,
                    &obstacles,
                    parameters,
                )?;
                assert!(
                    body.velocity.length() <= 50.0,
                    "{target}: passive collision gained speed"
                );
            }
            Ok((body, plants))
        };
        let substeps = parameters().physics.substeps;
        let (coarse, coarse_plants) = integrate(substeps)?;
        let (fine, fine_plants) = integrate(4 * substeps)?;
        let position_error = coarse.position.distance(fine.position);
        let velocity_error = coarse.velocity.distance(fine.velocity);
        eprintln!("{target}: position error {position_error}, velocity error {velocity_error}");
        assert!(
            position_error < 0.1,
            "{target}: position error {position_error}"
        );
        assert!(
            velocity_error < 0.1,
            "{target}: velocity error {velocity_error}"
        );
        for (coarse, fine) in coarse_plants.iter().zip(&fine_plants) {
            assert!(coarse.position.distance(fine.position) < 0.1);
            assert!(coarse.velocity.distance(fine.velocity) < 0.1);
        }
    }
    Ok(())
}

#[test]
fn crowded_plants_relax_under_fixed_steps() -> Result<()> {
    let parameters = <Config>::default().parameters();
    let mut plants: Vec<_> = (0..64)
        .map(|index| {
            Plant::new(
                index + 1,
                DVec2::new((index % 8) as f64 - 3.5, (index / 8) as f64 - 3.5) * 15.0,
                1.0,
            )
        })
        .collect();
    let mut peak_speed = 0.0_f64;
    for _ in 0..100 {
        advance(&mut [], &mut plants, &[], parameters)?;
        for plant in &plants {
            assert!(
                plant
                    .position
                    .abs()
                    .cmplt(parameters.world.half_size())
                    .all()
            );
            peak_speed = peak_speed.max(plant.velocity.length());
            assert!(plant.velocity.length() < 200.0);
        }
    }
    let final_speed = plants
        .iter()
        .map(|plant| plant.velocity.length())
        .fold(0.0_f64, f64::max);
    eprintln!("crowded plants: peak speed {peak_speed}, final speed {final_speed}");
    assert!(final_speed < 0.1);
    Ok(())
}

#[test]
fn damped_motion_converges_as_substeps_increase() -> Result<()> {
    let error = |substeps| -> Result<f64> {
        let mut parameters = parameters();
        parameters.physics.time_step = 1.0;
        parameters.physics.substeps = substeps;
        parameters.body.linear_drag = 1.0;
        let mut body = Body::new(DVec2::ZERO, 1.0);
        body.velocity = DVec2::X;
        advance(
            &mut [(&mut body, MotorAction::default())],
            &mut [],
            &[],
            parameters,
        )?;
        Ok((body.velocity.x - (-1.0_f64).exp()).abs())
    };
    let coarse = error(4)?;
    let fine = error(8)?;
    let reference = error(64)?;
    assert!(fine < coarse / 12.0);
    assert!(reference < fine / 100.0);
    Ok(())
}
