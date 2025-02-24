use std::{
    collections::hash_map::{Entry, HashMap},
    sync::{Arc, Mutex, Weak},
};

use anyhow::{Result, anyhow};
use candle::Tensor;

use crate::agent::{Brain, Perception};
use nevo_core::Candle as Cx;

use super::model::{AnimalAction, AnimalModel, WorldModel};

pub struct AnimalControl {
    brain: Brain,
    action: Weak<Mutex<Option<AnimalAction>>>,
}

impl AnimalControl {
    fn process(&mut self, cx: &mut Cx, body: &AnimalModel, world: &WorldModel) -> Result<()> {
        unimplemented!()
    }
}

#[derive(Default)]
pub struct WorldControl {
    animals: HashMap<usize, AnimalControl>,
}

impl WorldControl {
    pub fn sync(&mut self, cx: &mut Cx, model: &WorldModel) -> Result<()> {
        // Remove controls if associated model is removed
        self.animals.retain(|_, a| a.action.upgrade().is_some());

        // Add controls for new models
        for a in model.animals.iter() {
            if let Entry::Vacant(e) = self.animals.entry(a.key()) {
                e.insert(AnimalControl {
                    brain: Brain::new(&a.genome, cx)?,
                    action: Arc::downgrade(&a.action),
                });
            }
        }

        Ok(())
    }

    pub fn process(&mut self, cx: &mut Cx, model: &WorldModel) -> Result<()> {
        for body in &model.animals {
            let key = body.key();
            let animal = self.animals.get_mut(&key).ok_or_else(|| {
                anyhow!("No control for animal {key:?}. Maybe WorldControl was not synced.")
            })?;
            animal.process(cx, body, model)?;
        }
        Ok(())
    }
}
