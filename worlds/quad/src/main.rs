mod animal;
mod ui;
mod world;

use std::sync::{Arc, RwLock};

use animal::{MindConfig, VisionConfig, VisionLayerConfig};
use anyhow::Result;
use candle::{DType, Device};
use eframe::egui;
use glam::UVec2;
use rand::{rngs::SmallRng, SeedableRng};
use world::view::WorldView;

use self::{
    ui::App,
    world::{World, WorldConfig},
};
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
    let world = World::new(&mut cx, config, mind)?;
    println!("World is created");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Nevo Viewer",
        native_options,
        Box::new(|cc| {
            // let ctx = cc.egui_ctx.clone();
            // ctx.request_repaint();
            Ok(Box::new(App::new(cc, WorldView::new(&world))?))
        }),
    )
    .unwrap();

    Ok(())
}
