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
    type Config: Default;

    /// Compute output and an owned, validated state update without changing this
    /// mind. The caller supplies a private context when random draws must roll back.
    fn compute_update(
        &self,
        input: Self::Input,
        config: &Self::Config,
        cx: &mut C,
    ) -> Result<(Self::Output, Self::Update)>;

    /// Install a prepared update on the unchanged source mind. This must not fail,
    /// draw randomness, or perform further fallible computation.
    fn apply_update(&mut self, update: Self::Update);

    fn process(
        &mut self,
        input: Self::Input,
        config: &Self::Config,
        cx: &mut C,
    ) -> Result<Self::Output> {
        let (output, update) = self.compute_update(input, config, cx)?;
        self.apply_update(update);
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        Ok(())
    }
}
