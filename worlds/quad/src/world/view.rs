use std::sync::{RwLock, Weak};

use glam::UVec2;

use super::{AnimalModel, Block, Plant, WorldModel};

pub struct WorldView {
    model: Weak<RwLock<WorldModel>>,
}

pub trait Colored {
    fn color(&self) -> [u8; 3];
}

impl Colored for Block {
    fn color(&self) -> [u8; 3] {
        match self {
            Self::Ground => [0, 0x7f, 0],
            Self::Water => [0, 0, 0x7f],
        }
    }
}

impl Colored for Plant {
    fn color(&self) -> [u8; 3] {
        [0, 255, 0]
    }
}

impl Colored for AnimalModel {
    fn color(&self) -> [u8; 3] {
        [255, 0, 0]
    }
}

impl WorldModel {
    pub fn bitmap(&self) -> Vec<u8> {
        let mut data: Vec<u8> = self.blocks.items().iter().flat_map(Block::color).collect();
        let mut put_color = |pos: UVec2, color: [u8; 3]| {
            let offset = ((pos.y * self.blocks.size().x + pos.x) * 3) as usize;
            data[offset..(offset + 3)].copy_from_slice(&color);
        };
        for plant in &self.plants {
            put_color(plant.pos, plant.color())
        }
        for animal in &self.animals {
            put_color(animal.pos, animal.color())
        }
        data
    }
}
