//! Stateful thought through explicit input/output types.

pub mod animal;
pub mod rnn;

use anyhow::Result;

use crate::context::Context;

/// A replaceable mind; each implementation chooses its perception and action types.
pub trait Mind<C: Context + ?Sized> {
    type Input;
    type Output;
    type Update;

    /// Compute output and an owned, validated state update without changing this
    /// mind. The caller supplies a private context when random draws must roll back.
    fn prepare(&self, cx: &mut C, input: Self::Input) -> Result<(Self::Output, Self::Update)>;

    /// Install a prepared update on the unchanged source mind. This must not fail,
    /// draw randomness, or perform further fallible computation.
    fn apply(&mut self, update: Self::Update);

    fn process(&mut self, cx: &mut C, input: Self::Input) -> Result<Self::Output> {
        let (output, update) = self.prepare(cx, input)?;
        self.apply(update);
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
