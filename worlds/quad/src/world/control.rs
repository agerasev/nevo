use std::{
    collections::hash_map::{Entry, HashMap},
    sync::{Arc, Mutex, RwLock, Weak},
};

use anyhow::Result;

use crate::agent::Brain;
use nevo_core::Candle as Cx;

use super::model::{AnimalAction, WorldModel};

pub struct AnimalControl {
    brain: Brain,
    action: Weak<Mutex<Option<AnimalAction>>>,
}

#[derive(Default)]
pub struct WorldControl {
    animals: HashMap<*const Mutex<Option<AnimalAction>>, Arc<Mutex<AnimalControl>>>,
}

impl WorldControl {
    fn sync(&mut self, model: &RwLock<WorldModel>, cx: &mut Cx) -> Result<()> {
        // Remove controls if associated model is removed
        self.animals
            .retain(|_, a| a.lock().unwrap().action.upgrade().is_some());

        // Add controls for new models
        for a in model.read().unwrap().animals.iter() {
            if let Entry::Vacant(e) = self.animals.entry(Arc::as_ptr(&a.action)) {
                e.insert(Arc::new(Mutex::new(AnimalControl {
                    brain: Brain::new(&a.genome, cx)?,
                    action: Arc::downgrade(&a.action),
                })));
            }
        }

        Ok(())
    }
}
