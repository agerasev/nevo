mod view;

use anyhow::Result;
use glam::UVec2;
use rand::{distributions::Uniform, Rng};
use rand_distr::Distribution;

use crate::agent::{
    AgentConfig, AnimalBrain, AnimalGenome, MindConfig, VisionConfig, VisionLayerConfig,
};
use nevo_core::{Candle as Cx, Context, Evolving};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Block {
    Ground,
}

pub struct Plant {
    pos: UVec2,
    mass: f32,
}

pub struct Animal {
    pos: UVec2,
    mass: f32,
    brain: AnimalBrain,
}

pub struct WorldConfig {
    pub size: UVec2,
    pub n_plants: usize,
    pub n_animals: usize,
}

pub struct World {
    size: UVec2,
    blocks: Vec<Block>,
    plants: Vec<Plant>,
    animals: Vec<Animal>,
}

impl World {
    pub fn new(
        cx: &mut Cx,
        WorldConfig {
            size,
            n_plants,
            n_animals,
        }: WorldConfig,
        mind: MindConfig,
    ) -> Result<Self> {
        let agent = AgentConfig {
            vision_size: 16,
            vision_channels: 3,
            status_dim: 12,
            out_dim: 8,
        };

        let blocks = (0..(size.x * size.y)).map(|_| Block::Ground).collect();
        let plants = (0..n_plants)
            .map(|_| Plant {
                pos: cx.rng().sample(AxisAlignedUniform(UVec2::ZERO, size)),
                mass: cx.rng().sample(Uniform::new(0.1, 10.0)),
            })
            .collect();
        let animals = (0..n_animals)
            .map(|_| {
                let genome = AnimalGenome::new(cx, agent.clone(), mind.clone())?;
                Ok(Animal {
                    pos: cx.rng().sample(AxisAlignedUniform(UVec2::ZERO, size)),
                    mass: cx.rng().sample(Uniform::new(0.1, 1.0)),
                    brain: AnimalBrain::instantiate(&genome, cx)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            size,
            blocks,
            plants,
            animals,
        })
    }

    pub fn step(&mut self, cx: &mut Cx) {
        unimplemented!()
    }
}

pub struct AxisAlignedUniform<T>(pub T, pub T);

impl Distribution<UVec2> for AxisAlignedUniform<UVec2> {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> UVec2 {
        UVec2::from([
            rng.sample(Uniform::new(self.0.x, self.1.x)),
            rng.sample(Uniform::new(self.0.y, self.1.y)),
        ])
    }
}
