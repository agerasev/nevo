//! Stateful computation without environment-specific sensing or action semantics.

pub mod rnn;

use anyhow::Result;

use crate::context::Context;

/// A replaceable controller; each implementation chooses its signal types.
pub trait Mind<C: Context + ?Sized> {
    type Input;
    type Output;

    fn process(&mut self, cx: &mut C, input: Self::Input) -> Result<Self::Output>;
}
