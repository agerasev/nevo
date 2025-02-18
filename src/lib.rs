pub mod nn;

use anyhow::Result;
use candle::{DType, Device};
use rand::{Rng, RngCore};

/// Simulation context.
pub trait Context {
    type Rng: Rng + Send + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;

    type DType: Clone + Send + Sized;
    type Device: Clone + Send + Sized;
    fn dtype(&self) -> Self::DType;
    fn device(&self) -> Self::Device;
}

pub struct Candle<R: Rng + Send + ?Sized = dyn RngCore + Send> {
    pub dtype: DType,
    pub device: Device,
    pub rng: R,
}

impl<R: Rng + Send + ?Sized> Context for Candle<R> {
    type Rng = R;
    fn rng(&mut self) -> &mut Self::Rng {
        &mut self.rng
    }

    type DType = DType;
    type Device = Device;
    fn dtype(&self) -> Self::DType {
        self.dtype
    }
    fn device(&self) -> Self::Device {
        self.device.clone()
    }
}

/// Stateful agent.
pub trait Agent<C: Context + ?Sized> {
    type Input;
    type Output;

    /// Perform single step of processing.
    fn process(&mut self, cx: &mut C, input: Self::Input) -> Result<Self::Output>;
}

pub trait Genome<C: Context + ?Sized>: Clone + Sized {}

pub trait Mutate<C: Context + ?Sized, P: Clone>: Genome<C> {
    fn mutate(&mut self, param: &P, cx: &mut C) -> Result<()>;
}

pub trait Sexual<C: Context + ?Sized>: Genome<C> {
    fn recombine(&self, other: &Self, cx: &mut C) -> Result<Option<Self>>;
}

pub trait Evolving<C: Context + ?Sized>: Sized {
    type Genome: Genome<C>;
    fn genome(&self) -> Self::Genome;
    fn instantiate(gen: &Self::Genome, cx: &mut C) -> Result<Self>;
}
