//! Stateful actors and composable populations, independent of physical bodies.

mod handle;
pub mod herbivore;
pub mod population;

pub use handle::{Handle, HandleAllocator};

use anyhow::Result;

use crate::context::Context;

/// An observation may update state without requiring an immediate action.
pub trait Actor<C: Context + ?Sized> {
    type Observation;
    type Action;

    fn observe(&mut self, cx: &mut C, observation: Self::Observation) -> Result<()>;
    fn act(&mut self, cx: &mut C) -> Result<Self::Action>;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
