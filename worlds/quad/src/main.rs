mod agent;
mod world;

use std::{
    net::TcpListener,
    sync::{Arc, Mutex},
    thread::spawn,
};

use agent::{MindConfig, VisionConfig, VisionLayerConfig};
use anyhow::Result;
use candle::{DType, Device};
use glam::UVec2;
use nevo_api::{ControlMessage, MessageReader, MessageWriter};
use rand::{rngs::SmallRng, SeedableRng};

use self::world::{World, WorldConfig};
use nevo_core::Candle as Cx;

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
    let world = Arc::new(Mutex::new(World::new(&mut cx, config, mind)?));
    println!("World is created");

    let addr = ("0.0.0.0", 3399);
    println!("API is listening on {addr:?}");
    for accept in TcpListener::bind(addr)?.incoming() {
        let world = world.clone();
        spawn(move || {
            match (|| -> Result<()> {
                let socket = accept?;
                let mut reader = MessageReader::new(socket.try_clone()?);
                let mut writer = MessageWriter::new(socket);
                while let Some(msg) = reader.read_message()? {
                    match msg {
                        ControlMessage::Show => {
                            for view in world.lock().expect("World mutex is poisoned").view() {
                                writer.write_message(&view)?;
                            }
                        }
                    }
                }
                Ok(())
            })() {
                Ok(()) => (),
                Err(e) => eprintln!("Communication through API error: {e}"),
            }
        });
    }

    Ok(())
}
