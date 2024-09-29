pub mod nn;

use anyhow::Result;
use candle::{DType, Device};
use rand::Rng;

/// Simulation context.
pub trait Context {
    type Rng: Rng + Send + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;
    fn dtype(&self) -> DType;
    fn device(&self) -> Device;
}

pub struct Cx<R: Rng + Send + ?Sized> {
    pub dtype: DType,
    pub device: Device,
    pub rng: R,
}

impl<R: Rng + Send + ?Sized> Context for Cx<R> {
    type Rng = R;
    fn rng(&mut self) -> &mut Self::Rng {
        &mut self.rng
    }
    fn dtype(&self) -> DType {
        self.dtype
    }
    fn device(&self) -> Device {
        self.device.clone()
    }
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
    fn instance<C: Context>(cx: &mut C, genome: &Self::Genome) -> Result<Self>;
}
