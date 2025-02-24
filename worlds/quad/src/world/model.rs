use std::sync::{Arc, Mutex};

use anyhow::Result;
use glam::{IVec2, UVec2};
use rand::Rng;

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
    pub growth: f32,
}

impl Plant {
    pub fn new(pos: UVec2) -> Self {
        Plant { pos, growth: 0.0 }
    }
}

#[derive(Default, Debug)]
pub struct AnimalAction {
    pub motion: IVec2,
}

pub struct AnimalModel {
    pub genome: MindGenome,
    pub pos: UVec2,
    pub age: f32,
    pub hunger: f32,
    pub action: Arc<Mutex<Option<AnimalAction>>>,
}

impl AnimalModel {
    pub fn new(pos: UVec2, genome: MindGenome) -> Result<Self> {
        Ok(AnimalModel {
            genome,
            pos,
            age: 0.0,
            hunger: 0.0,
            action: Default::default(),
        })
    }

    pub fn key(&self) -> usize {
        Arc::as_ptr(&self.action).addr()
    }
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
            status_dim: 1,
            out_dim: 5,
        }
    }

    pub fn new(cx: &mut Cx, config: WorldConfig, mind: MindConfig) -> Result<Self> {
        let blocks = Grid2::fill(config.size, Block::Ground);
        let plants = (0..config.n_plants)
            .map(|_| {
                Plant::new(
                    cx.rng()
                        .sample(AxisAlignedUniform(UVec2::ZERO, config.size)),
                )
            })
            .collect();
        let animals = (0..config.n_animals)
            .map(|_| {
                AnimalModel::new(
                    cx.rng()
                        .sample(AxisAlignedUniform(UVec2::ZERO, config.size)),
                    MindGenome::new(cx, Self::animal_config(), mind.clone())?,
                )
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
