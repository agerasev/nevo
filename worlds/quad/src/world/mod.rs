mod view;

use std::{
    collections::hash_map::Entry,
    sync::{Arc, Mutex, RwLock, Weak},
};

use anyhow::Result;
use eframe::egui::ahash::HashMap;
use glam::UVec2;
use rand::{distributions::Uniform, Rng};

use crate::animal::{AgentConfig, AgentOutput, AnimalBrain, AnimalGenome, MindConfig};
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
    pos: UVec2,
    mass: f32,
}

#[derive(Default, Debug)]
pub struct AnimalInteraction {
    action: Option<AgentOutput>,
}

pub struct AnimalModel {
    pos: UVec2,
    mass: f32,
    genome: AnimalGenome,
    interaction: Arc<Mutex<AnimalInteraction>>,
}

pub struct AnimalControl {
    brain: AnimalBrain,
    interaction: Weak<Mutex<AnimalInteraction>>,
}

pub struct WorldConfig {
    pub size: UVec2,
    pub n_plants: usize,
    pub n_animals: usize,
}

pub struct WorldModel {
    blocks: Grid2<Block>,
    plants: Vec<Plant>,
    animals: Vec<AnimalModel>,
}

#[derive(Default)]
pub struct WorldControl {
    animals: HashMap<*const Mutex<AnimalInteraction>, Arc<Mutex<AnimalControl>>>,
}

pub struct World {
    config: WorldConfig,
    model: Arc<RwLock<WorldModel>>,
    control: WorldControl,
}

impl World {
    pub fn new(cx: &mut Cx, config: WorldConfig, mind: MindConfig) -> Result<Self> {
        let agent = AgentConfig {
            vision_size: 16,
            vision_channels: 3,
            status_dim: 12,
            out_dim: 8,
        };

        let model = {
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
                        genome: AnimalGenome::new(cx, agent.clone(), mind.clone())?,
                        interaction: Default::default(),
                    })
                })
                .collect::<Result<_>>()?;
            WorldModel {
                blocks,
                plants,
                animals,
            }
        };

        Ok(World {
            config,
            model: Arc::new(RwLock::new(model)),
            control: WorldControl::default(),
        })
    }

    fn sync_control(&mut self, cx: &mut Cx) -> Result<()> {
        // Remove controls if associated model is removed
        self.control
            .animals
            .retain(|_, a| a.lock().unwrap().interaction.upgrade().is_some());

        // Add controls for new models
        for a in self.model.read().unwrap().animals.iter() {
            if let Entry::Vacant(e) = self.control.animals.entry(Arc::as_ptr(&a.interaction)) {
                e.insert(Arc::new(Mutex::new(AnimalControl {
                    brain: AnimalBrain::new(&a.genome, cx)?,
                    interaction: Arc::downgrade(&a.interaction),
                })));
            }
        }

        Ok(())
    }

    pub fn step(&mut self, cx: &mut Cx) {
        unimplemented!()
    }
}
