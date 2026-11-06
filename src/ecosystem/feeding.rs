//! Inelastic assimilation: edible mass, linear momentum and spin join the animal.

use anyhow::{Result, ensure};

use super::{Body, Plant};

pub(super) fn assimilate(
    body: &mut Body,
    plant: &Plant,
    efficiency: f64,
    mass_per_energy: f64,
) -> Result<f64> {
    let gain = efficiency * plant.energy;
    if gain == 0.0 {
        return Ok(0.0);
    }
    let energy = body.energy + gain;
    ensure!(
        energy.is_finite() && energy > 0.0,
        "invalid energy after assimilation"
    );
    let fraction = gain / energy;
    let relative_position = plant.position - body.position;
    let relative_velocity = plant.velocity - body.velocity;
    let reduced_mass = body.mass(mass_per_energy) * fraction;
    let angular_momentum = body.inertia(mass_per_energy) * body.angular_velocity
        + efficiency * plant.inertia(mass_per_energy) * plant.angular_velocity
        + reduced_mass * relative_position.perp_dot(relative_velocity);
    body.position += fraction * relative_position;
    body.velocity += fraction * relative_velocity;
    body.energy = energy;
    body.angular_velocity = angular_momentum / body.inertia(mass_per_energy);
    ensure!(
        body.position.is_finite() && body.velocity.is_finite() && body.angular_velocity.is_finite(),
        "non-finite motion after assimilation"
    );
    Ok(gain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn assimilation_transfers_only_edible_mass_and_its_momentum() {
        let mut animal = Body::new(DVec2::new(-2.0, 0.0), 0.5);
        animal.velocity = DVec2::new(2.0, 1.0);
        animal.angular_velocity = 0.2;
        let mut plant = Plant::new(1, DVec2::new(2.0, 1.0), 1.0);
        plant.velocity = DVec2::new(-1.0, 2.0);
        plant.angular_velocity = -0.1;
        let efficiency = 0.25;
        let mass_unit = 3.0;
        let m = animal.mass(mass_unit);
        let edible_mass = efficiency * plant.mass(mass_unit);
        let momentum = m * animal.velocity + edible_mass * plant.velocity;
        let center = (m * animal.position + edible_mass * plant.position) / (m + edible_mass);
        let angular = animal.inertia(mass_unit) * animal.angular_velocity
            + m * (animal.position - center).perp_dot(animal.velocity)
            + efficiency * plant.inertia(mass_unit) * plant.angular_velocity
            + edible_mass * (plant.position - center).perp_dot(plant.velocity);
        assert_eq!(
            assimilate(&mut animal, &plant, efficiency, mass_unit).unwrap(),
            0.25
        );
        assert!((animal.mass(mass_unit) * animal.velocity).distance(momentum) < 1e-12);
        assert!(animal.position.distance(center) < 1e-12);
        assert!((animal.inertia(mass_unit) * animal.angular_velocity - angular).abs() < 1e-12);
        assert_eq!(animal.energy, 0.75);
    }
}
