pub mod nn;

use anyhow::Result;
use candle::{DType, Device};
use rand::Rng;

/// Simulation context.
pub trait Context {
    type Rng: Rng + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;
    fn dtype(&self) -> DType;
    fn device(&self) -> Device;
}

/// Stateful agent.
pub trait Agent {
    type Input;
    type Output;

    /// Perform single step of processing.
    fn process<C: Context>(&mut self, cx: &mut C, input: Self::Input) -> Result<Self::Output>;
}

pub trait Genome: Clone + Sized {}

pub trait Mutate<P: Clone> {
    fn mutate<R: Rng + ?Sized>(&mut self, param: &P, rng: &mut R) -> Result<()>;
}

pub trait Sexual: Genome {
    fn recombine<R: Rng + ?Sized>(&self, other: &Self, rng: &mut R) -> Result<Option<Self>>;
}

pub trait Evolving: Sized {
    type Genome: Genome;
    fn genome(&self) -> Self::Genome;
    fn instance<C: Context>(genome: &Self::Genome, cx: &mut C) -> Result<Self>;
}

pub trait System {
    fn step<C: Context>(&mut self, cx: &mut C) -> Result<()>;
}
