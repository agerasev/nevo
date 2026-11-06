//! Synchronous disk dynamics with fixed-step `phy` RK4 integration.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use glam::Vec2;
use phy::{Rk4, Solver, System, Var, Visitor};

use super::{Body, MotorAction, Obstacle, Plant, body, config::Parameters};

pub(super) use super::body::Motion;

type Integrator = Rk4<f32>;

pub(super) struct Step {
    pub motions: Vec<Motion>,
    /// Animal and plant indices that touched during accepted integration stages.
    pub contacts: BTreeSet<(usize, usize)>,
}

struct Particle {
    position: Var<Vec2, Integrator>,
    velocity: Var<Vec2, Integrator>,
    heading: Var<f32, Integrator>,
    angular_velocity: Var<f32, Integrator>,
    distance: Var<f32, Integrator>,
    work: Var<f32, Integrator>,
    force_squared_time: Var<f32, Integrator>,
    torque_squared_time: Var<f32, Integrator>,
    mass: f32,
    radius: f32,
    linear_drag: f32,
    angular_drag: f32,
    acceleration: f32,
    angular_acceleration: f32,
    force: f32,
    torque: f32,
}

impl Particle {
    #[allow(clippy::too_many_arguments)]
    fn new(
        position: Vec2,
        velocity: Vec2,
        heading: f32,
        angular_velocity: f32,
        mass: f32,
        radius: f32,
        inertia: f32,
        linear_drag: f32,
        angular_drag: f32,
        acceleration: f32,
        angular_acceleration: f32,
    ) -> Result<Self> {
        ensure!(
            position.is_finite()
                && velocity.is_finite()
                && heading.is_finite()
                && angular_velocity.is_finite()
                && mass.is_finite()
                && mass >= 0.0,
            "non-finite physical disk state"
        );
        ensure!(
            mass == 0.0
                || (radius.is_finite() && radius > 0.0 && inertia.is_finite() && inertia > 0.0),
            "active disks require positive finite radius and inertia"
        );
        let force = mass * acceleration;
        let torque = inertia * angular_acceleration;
        ensure!(
            force.is_finite() && torque.is_finite(),
            "non-finite motor effort"
        );
        Ok(Self {
            position: Var::new(position),
            velocity: Var::new(if mass > 0.0 { velocity } else { Vec2::ZERO }),
            heading: Var::new(heading),
            angular_velocity: Var::new(if mass > 0.0 { angular_velocity } else { 0.0 }),
            distance: Var::default(),
            work: Var::default(),
            force_squared_time: Var::default(),
            torque_squared_time: Var::default(),
            mass,
            radius,
            linear_drag,
            angular_drag,
            acceleration,
            angular_acceleration,
            force,
            torque,
        })
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.position.is_finite()
                && self.velocity.is_finite()
                && self.heading.is_finite()
                && self.angular_velocity.is_finite()
                && self.distance.is_finite()
                && self.work.is_finite()
                && self.force_squared_time.is_finite()
                && self.torque_squared_time.is_finite(),
            "non-finite integrated disk state"
        );
        Ok(())
    }

    fn motion(&self) -> Motion {
        Motion {
            distance: *self.distance,
            mechanical_work: *self.work,
            force_squared_time: *self.force_squared_time,
            torque_squared_time: *self.torque_squared_time,
        }
    }
}

struct Scene<'a> {
    particles: Vec<Particle>,
    animals: usize,
    obstacles: &'a [Obstacle],
    parameters: Parameters,
    contacts: BTreeSet<(usize, usize)>,
    error: Option<String>,
}

impl Scene<'_> {
    fn contact_acceleration(&self, penetration: f32, relative_velocity: f32) -> Result<f32> {
        let physics = self.parameters.physics;
        let acceleration = physics.contact_frequency.powi(2) * penetration
            - (2.0 * physics.contact_damping * physics.contact_frequency) * relative_velocity;
        ensure!(acceleration.is_finite(), "non-finite contact acceleration");
        Ok(acceleration.max(0.0))
    }

    fn pair(&mut self, a: usize, b: usize) -> Result<()> {
        let pa = &self.particles[a];
        let pb = &self.particles[b];
        let Some(contact) =
            geom2::contact::disks(*pa.position, pa.radius, *pb.position, pb.radius)?
        else {
            return Ok(());
        };
        if a < self.animals && b >= self.animals {
            self.contacts.insert((a, b - self.animals));
        }
        let relative = *pb.velocity - *pa.velocity;
        let normal = contact
            .normal
            .unwrap_or_else(|| coincident_normal(relative));
        let acceleration = self.contact_acceleration(contact.penetration, relative.dot(normal))?;
        // This form avoids overflow in the sum of masses.
        let smaller = pa.mass.min(pb.mass);
        let larger = pa.mass.max(pb.mass);
        let reduced_mass = smaller / (1.0 + smaller / larger);
        ensure!(reduced_mass > 0.0, "contact reduced mass underflowed");
        let force = reduced_mass * acceleration;
        ensure!(force.is_finite(), "non-finite contact force");
        let acceleration_a = (force / pa.mass) * normal;
        let acceleration_b = (force / pb.mass) * normal;
        self.particles[a].velocity.deriv -= acceleration_a;
        self.particles[b].velocity.deriv += acceleration_b;
        Ok(())
    }

    fn derivatives(&mut self) -> Result<()> {
        for particle in &mut self.particles {
            particle.validate()?;
            if particle.mass == 0.0 {
                continue;
            }
            let forward = Vec2::from_angle(*particle.heading);
            particle.position.deriv = *particle.velocity;
            particle.velocity.deriv =
                particle.acceleration * forward - particle.linear_drag * *particle.velocity;
            particle.heading.deriv = *particle.angular_velocity;
            particle.angular_velocity.deriv =
                particle.angular_acceleration - particle.angular_drag * *particle.angular_velocity;
            particle.distance.deriv = length(*particle.velocity);
            let linear_power = particle.force * forward.dot(*particle.velocity);
            let angular_power = particle.torque * *particle.angular_velocity;
            ensure!(
                linear_power.is_finite() && angular_power.is_finite(),
                "non-finite motor power"
            );
            particle.work.deriv = linear_power.max(0.0) + angular_power.max(0.0);
            particle.force_squared_time.deriv = particle.force * particle.force;
            particle.torque_squared_time.deriv = particle.torque * particle.torque;
        }

        // A disk pair can touch only in the same or adjacent center buckets.
        // The index accelerates geometry without discretizing positions or forces.
        let max_radius = self
            .particles
            .iter()
            .filter(|particle| particle.mass > 0.0)
            .map(|particle| particle.radius)
            .fold(0.0_f32, f32::max);
        if max_radius > 0.0 {
            let cell_size = 2.0 * max_radius;
            ensure!(cell_size.is_finite(), "disk diameter is too large");
            let mut bins = BTreeMap::<(i64, i64), Vec<usize>>::new();
            for (index, particle) in self.particles.iter().enumerate() {
                if particle.mass > 0.0 {
                    bins.entry(cell(*particle.position, cell_size)?)
                        .or_default()
                        .push(index);
                }
            }
            for a in 0..self.particles.len() {
                if self.particles[a].mass == 0.0 {
                    continue;
                }
                let (x, y) = cell(*self.particles[a].position, cell_size)?;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if let Some(candidates) = bins.get(&(x + dx, y + dy)) {
                            for &b in candidates {
                                if b > a {
                                    self.pair(a, b)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        let half_size = self.parameters.world.half_size();
        for index in 0..self.particles.len() {
            if self.particles[index].mass == 0.0 {
                continue;
            }
            for obstacle in self.obstacles {
                let particle = &self.particles[index];
                if let Some(contact) = geom2::contact::disks(
                    *particle.position,
                    particle.radius,
                    obstacle.position,
                    obstacle.radius,
                )? {
                    let normal = contact
                        .normal
                        .unwrap_or_else(|| coincident_normal(-*particle.velocity));
                    let acceleration = self.contact_acceleration(
                        contact.penetration,
                        -particle.velocity.dot(normal),
                    )?;
                    self.particles[index].velocity.deriv -= acceleration * normal;
                }
            }
            for axis in 0..2 {
                for sign in [-1.0, 1.0] {
                    let particle = &self.particles[index];
                    let penetration =
                        particle.radius + sign * particle.position[axis] - half_size[axis];
                    if penetration >= 0.0 {
                        let acceleration = self
                            .contact_acceleration(penetration, -sign * particle.velocity[axis])?;
                        self.particles[index].velocity.deriv[axis] -= sign * acceleration;
                    }
                }
            }
        }

        for particle in &self.particles {
            if particle.mass == 0.0 {
                continue;
            }
            ensure!(
                particle.velocity.deriv.is_finite()
                    && particle.angular_velocity.deriv.is_finite()
                    && particle.distance.deriv.is_finite()
                    && particle.work.deriv.is_finite()
                    && particle.force_squared_time.deriv.is_finite()
                    && particle.torque_squared_time.deriv.is_finite(),
                "non-finite physical derivative"
            );
        }
        Ok(())
    }
}

impl System<Integrator, f32> for Scene<'_> {
    fn compute_derivs(&mut self, _: &<Integrator as Solver<f32>>::Context) {
        if self.error.is_none()
            && let Err(error) = self.derivatives()
        {
            self.error = Some(error.to_string());
        }
    }

    fn visit_vars<V: Visitor<Integrator, f32>>(&mut self, visitor: &mut V) {
        for particle in &mut self.particles {
            visitor.apply(&mut particle.position);
            visitor.apply(&mut particle.velocity);
            visitor.apply(&mut particle.heading);
            visitor.apply(&mut particle.angular_velocity);
            visitor.apply(&mut particle.distance);
            visitor.apply(&mut particle.work);
            visitor.apply(&mut particle.force_squared_time);
            visitor.apply(&mut particle.torque_squared_time);
        }
    }
}

fn length(vector: Vec2) -> f32 {
    vector.x.hypot(vector.y)
}

fn coincident_normal(relative_velocity: Vec2) -> Vec2 {
    let speed = length(relative_velocity);
    if speed > 0.0 && speed.is_finite() {
        -relative_velocity / speed
    } else {
        Vec2::X
    }
}

fn cell(position: Vec2, width: f32) -> Result<(i64, i64)> {
    let coordinates = (position / width).floor();
    // Leave room for neighboring bucket coordinates, including f32 rounding.
    let limit = (i64::MAX / 2) as f32;
    ensure!(
        coordinates.is_finite() && coordinates.abs().cmplt(Vec2::splat(limit)).all(),
        "physical spatial index is out of range"
    );
    Ok((coordinates.x as i64, coordinates.y as i64))
}

pub(super) fn advance(
    animals: &mut [(&mut Body, MotorAction)],
    plants: &mut [Plant],
    obstacles: &[Obstacle],
    parameters: Parameters,
) -> Result<Step> {
    parameters.validate()?;
    super::obstacle::validate(obstacles, parameters.world)?;
    let physics = parameters.physics;
    let mut particles = Vec::with_capacity(animals.len() + plants.len());
    for (body, action) in animals.iter() {
        body.validate(parameters.world, parameters.body)?;
        ensure!(
            body.energy <= 0.0 || body.mass(physics.mass_per_energy) > 0.0,
            "animal mass underflowed"
        );
        ensure!(
            action.thrust.is_finite() && action.turn.is_finite(),
            "non-finite motor action"
        );
        particles.push(Particle::new(
            body.position,
            body.velocity,
            body.heading,
            body.angular_velocity,
            body.mass(physics.mass_per_energy),
            body.radius(),
            body.inertia(physics.mass_per_energy),
            parameters.body.linear_drag,
            parameters.body.angular_drag,
            action.thrust.clamp(0.0, 1.0) * parameters.body.max_acceleration,
            action.turn.clamp(-1.0, 1.0) * parameters.body.max_angular_acceleration,
        )?);
    }
    for plant in plants.iter() {
        ensure!(
            plant.energy.is_finite() && plant.energy >= 0.0,
            "invalid physical plant energy"
        );
        ensure!(
            plant.energy == 0.0 || plant.mass(physics.mass_per_energy) > 0.0,
            "plant mass underflowed"
        );
        particles.push(Particle::new(
            plant.position,
            plant.velocity,
            0.0,
            plant.angular_velocity,
            plant.mass(physics.mass_per_energy),
            plant.radius(),
            plant.inertia(physics.mass_per_energy),
            parameters.plant.linear_drag,
            parameters.plant.angular_drag,
            0.0,
            0.0,
        )?);
    }
    let mut scene = Scene {
        particles,
        animals: animals.len(),
        obstacles,
        parameters,
        contacts: BTreeSet::new(),
        error: None,
    };
    let step = physics.time_step / physics.substeps as f32;
    for _ in 0..physics.substeps {
        Integrator::default().solve_step(&mut scene, step);
        if let Some(error) = &scene.error {
            anyhow::bail!("{error}");
        }
        for particle in &mut scene.particles {
            particle.validate()?;
            *particle.heading = body::normalize_heading(*particle.heading);
        }
    }

    // Publish only after every fixed substep has succeeded.
    let motions = scene.particles[..animals.len()]
        .iter()
        .map(Particle::motion)
        .collect();
    for ((body, _), particle) in animals.iter_mut().zip(&scene.particles) {
        body.position = *particle.position;
        body.velocity = *particle.velocity;
        body.heading = *particle.heading;
        body.angular_velocity = *particle.angular_velocity;
    }
    for (plant, particle) in plants.iter_mut().zip(&scene.particles[animals.len()..]) {
        plant.position = *particle.position;
        plant.velocity = *particle.velocity;
        plant.angular_velocity = *particle.angular_velocity;
    }
    Ok(Step {
        motions,
        contacts: scene.contacts,
    })
}

#[cfg(test)]
mod tests;
