use crate::{
    agent::{AnimalBrain, AnimalGenome, MindConfig, VisionConfig, VisionLayerConfig},
    AgentConfig,
};
use anyhow::Result;
use nevo::{Candle as Cx, Context, Evolving};
use rand::{distributions::Uniform, Rng};
use vecmat::Vector;

pub type Pos = Vector<u32, 2>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Block {
    Ground,
}

pub struct Plant {
    pos: Pos,
    mass: f32,
}

pub struct Animal {
    pos: Pos,
    mass: f32,
    brain: AnimalBrain,
}

pub struct WorldConfig {
    pub size: Pos,
    pub n_plants: usize,
    pub n_animals: usize,
}

pub struct World {
    size: Pos,
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

        let blocks = (0..(size.x() * size.y())).map(|_| Block::Ground).collect();
        let plants = (0..n_plants)
            .map(|_| Plant {
                pos: sample_pos(cx.rng(), size),
                mass: cx.rng().sample(Uniform::new(0.1, 10.0)),
            })
            .collect();
        let animals = (0..n_animals)
            .map(|_| {
                let genome = AnimalGenome::new(cx, agent.clone(), mind.clone())?;
                Ok(Animal {
                    pos: sample_pos(cx.rng(), size),
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

    pub fn step(&mut self) {
        unimplemented!()
    }
}

fn sample_pos<R: Rng + ?Sized>(rng: &mut R, size: Pos) -> Pos {
    Pos::from([
        rng.sample(Uniform::new(0, size.x())),
        rng.sample(Uniform::new(0, size.y())),
    ])
}
