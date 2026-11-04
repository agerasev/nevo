//! Representation-independent inheritance and mutation contracts.

use anyhow::Result;

use crate::context::Context;

pub trait Genome<C: Context + ?Sized>: Clone + Sized {}

pub trait Mutate<C: Context + ?Sized, P: Clone>: Genome<C> {
    fn mutate(&mut self, param: &P, cx: &mut C) -> Result<()>;
}
