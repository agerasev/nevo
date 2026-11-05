//! Independent random initialization, inheritance, and mutation contracts.

use anyhow::Result;

use crate::context::Context;

pub trait Genome<C: Context + ?Sized>: Clone + Sized {}

/// Construct random state from typed settings and a context, independently of inheritance.
pub trait RandomInit<C: Context + ?Sized>: Sized {
    type Config: Default + Clone + std::fmt::Debug;

    fn random(config: &Self::Config, cx: &mut C) -> Result<Self>;
}

pub trait Mutate<C: Context + ?Sized, P: Clone>: Genome<C> {
    fn mutate(&mut self, param: &P, cx: &mut C) -> Result<()>;
}
