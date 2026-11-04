//! Stateful thought and inheritance through explicit input/output types.

pub mod herbivore;
pub mod rnn;

use anyhow::Result;

use crate::context::Context;

/// A replaceable mind; each implementation chooses its perception and action types.
pub trait Mind<C: Context + ?Sized> {
    type Input;
    type Output;

    fn process(&mut self, cx: &mut C, input: Self::Input) -> Result<Self::Output>;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Construct fresh minds and inherit offspring without knowing physical bodies.
pub trait Heredity<C: Context + ?Sized, M> {
    fn immigrant(&self, cx: &mut C) -> Result<M>;
    fn offspring(&self, cx: &mut C, parent: &M) -> Result<M>;

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
