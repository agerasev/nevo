use std::sync::{Arc, Mutex};

use anyhow::Result;
use glam::{IVec2, UVec2};
use rand::{Rng, distributions::Uniform};

use crate::agent::{InteractionConfig, MindConfig, MindGenome};
use nevo_core::{Candle as Cx, Context};
use nevo_utils::{grid::Grid2, rand::AxisAlignedUniform};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Block {
    Ground,
    Water,
}

#[derive(Default, Debug)]
pub struct Plant {
    pub pos: UVec2,
    pub mass: f32,
}

#[derive(Default, Debug)]
pub struct AnimalAction {
    pub motion: IVec2,
}

pub struct AnimalModel {
    pub pos: UVec2,
    pub mass: f32,
    pub genome: MindGenome,
    pub action: Arc<Mutex<Option<AnimalAction>>>,
}

#[derive(Clone, Debug)]
pub struct WorldConfig {
    pub size: UVec2,
    pub n_plants: usize,
    pub n_animals: usize,
}

pub struct WorldModel {
    n_steps: u64,
    pub(super) blocks: Grid2<Block>,
    pub(super) plants: Vec<Plant>,
    pub(super) animals: Vec<AnimalModel>,
}

impl WorldModel {
    pub const fn animal_config() -> InteractionConfig {
        InteractionConfig {
            vision_size: 16,
            vision_channels: 3,
            status_dim: 12,
            out_dim: 8,
        }
    }

    pub fn new(cx: &mut Cx, config: WorldConfig, mind: MindConfig) -> Result<Self> {
        let blocks = Grid2::fill(config.size, Block::Ground);
        let plants = (0..config.n_plants)
            .map(|_| Plant {
                pos: cx
                    .rng()
                    .sample(AxisAlignedUniform(UVec2::ZERO, config.size)),
                mass: cx.rng().sample(Uniform::new(0.1, 10.0)),
            })
            .collect();
        let animals = (0..config.n_animals)
            .map(|_| {
                Ok(AnimalModel {
                    pos: cx
                        .rng()
                        .sample(AxisAlignedUniform(UVec2::ZERO, config.size)),
                    mass: cx.rng().sample(Uniform::new(0.1, 1.0)),
                    genome: MindGenome::new(cx, Self::animal_config(), mind.clone())?,
                    action: Default::default(),
                })
            })
            .collect::<Result<_>>()?;

        Ok(Self {
            n_steps: 0,
            blocks,
            plants,
            animals,
        })
    }

    pub fn state_hash(&self) -> u64 {
        self.n_steps
    }

    pub fn step(&mut self, _cx: &mut Cx) {
        unimplemented!()
    }
}
