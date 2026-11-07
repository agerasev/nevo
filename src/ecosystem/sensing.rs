//! Separate food and animal fields, expressed in the observer's forward/left axes.

use super::*;

fn field(
    body: &Body,
    sources: impl Iterator<Item = (Vec2, f32)>,
    config: SensingConfig,
    signal_scale: f32,
) -> (f32, Vec2) {
    if signal_scale == 0.0 {
        return (0.0, Vec2::ZERO);
    }
    let mut strength = 0.0;
    let mut gradient = Vec2::ZERO;
    for (position, mass) in sources {
        let displacement = (position - body.position) * config.distance_scale;
        let distance = displacement.length() + config.softening;
        strength += mass / distance;
        gradient += mass * displacement / (distance * distance * distance);
    }
    strength *= signal_scale;
    gradient *= signal_scale;
    gradient = if gradient.length() > config.gradient_dead_zone {
        body.world_to_body(gradient.normalize())
    } else {
        Vec2::ZERO
    };
    (strength, gradient)
}

impl<M, G, P, MC, IC> Ecosystem<M, G, P, MC, IC> {
    pub(super) fn sense(&self, observer: AnimalId) -> AnimalObservation {
        let body = &self.animals[&observer].body;
        let config = self.parameters.sensing;
        let (strength, gradient) = field(
            body,
            self.plants
                .iter()
                .map(|p| (p.position, p.energy / self.parameters.plant.max_energy)),
            config,
            config.signal_scale,
        );
        let (animal_strength, animal_gradient) = field(
            body,
            self.animals
                .iter()
                .filter(|(id, animal)| **id != observer && animal.body.energy > 0.0)
                .map(|(_, animal)| (animal.body.position, animal.body.energy)),
            config,
            config.animal_signal_scale,
        );
        AnimalObservation {
            strength,
            gradient,
            animal_strength,
            animal_gradient,
            velocity: body.world_to_body(body.velocity) * config.velocity_scale,
            angular_velocity: body.angular_velocity * config.angular_velocity_scale,
        }
    }
}
