pub mod control;
pub mod model;
pub mod view;

pub use self::{
    control::WorldControl,
    model::{WorldConfig, WorldModel},
};

use std::sync::{Arc, RwLock};

use anyhow::Result;

use crate::agent::MindConfig;
use nevo_core::Candle as Cx;

pub struct World {
    pub config: WorldConfig,
    pub model: Arc<RwLock<WorldModel>>,
    pub control: WorldControl,
}

impl World {
    pub fn new(cx: &mut Cx, config: WorldConfig, mind: MindConfig) -> Result<Self> {
        let model = WorldModel::new(cx, config.clone(), mind)?;

        Ok(World {
            config,
            model: Arc::new(RwLock::new(model)),
            control: WorldControl::default(),
        })
    }
}
