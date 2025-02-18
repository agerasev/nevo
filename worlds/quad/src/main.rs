mod agent;
mod world;

use agent::{MindConfig, VisionConfig, VisionLayerConfig};
use anyhow::Result;
use candle::{DType, Device, Tensor};
use glam::UVec2;
use rand::{rngs::SmallRng, SeedableRng};

use self::world::{World, WorldConfig};
use nevo_core::Candle as Cx;

#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub vision_channels: usize,
    pub vision_size: usize,
    pub status_dim: usize,
    pub out_dim: usize,
}

#[derive(Clone, Debug)]
pub struct AgentInput {
    /// shape: (1, vision_channels, vision_size, vision_size)
    pub vision: Tensor,
    /// shape: (1, status_dim)
    pub status: Tensor,
}

#[derive(Clone, Debug)]
pub struct AgentOutput {
    /// shape: (1, out_dim)
    pub action: Tensor,
}

fn main() -> Result<()> {
    let mut cx = Cx {
        dtype: DType::F32,
        device: Device::Cpu,
        rng: SmallRng::seed_from_u64(0xdeadbeef),
    };
    let mind = MindConfig {
        vision: VisionConfig {
            layers: vec![
                VisionLayerConfig {
                    kernel_size: 3,
                    out_channels: 8,
                    max_pool: 2,
                },
                VisionLayerConfig {
                    kernel_size: 3,
                    out_channels: 8,
                    max_pool: 2,
                },
            ],
        },
        mem_size: 256,
    };
    let config = WorldConfig {
        size: UVec2::from([256, 256]),
        n_plants: 1000,
        n_animals: 100,
    };
    let _world = World::new(&mut cx, config, mind)?;
    Ok(())
}
