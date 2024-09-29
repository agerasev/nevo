use crate::animal::Animal;
use rand::{distributions::Uniform, Rng};
use vecmat::vector::Vector2;

pub struct World {
    size: Vector2<f64>,
    animals: Vec<Animal>,
}

impl World {
    const ANIMAL_INIT_MEM_SIZE: usize = 4;

    pub fn new<R: Rng>(rng: &mut R, size: Vector2<f64>, count: usize) -> Self {
        let mut animals = Vec::new();
        for _ in 0..count {
            animals.push(Animal::new(
                Genome::new(Self::ANIMAL_INIT_MEM_SIZE, 1.0, rng),
                size * Vector2::init(|| rng.sample(Uniform::from(0.0..=1.0))),
            ));
        }
        Self { size, animals }
    }

    pub fn step(&mut self) {}
}
