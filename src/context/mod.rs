//! Execution context shared by agents, minds, and genome operations.

mod cpu;

use rand::Rng;

pub use cpu::Cpu;

/// Simulation context, retaining the draft's RNG and compute metadata contract.
pub trait Context {
    type Rng: Rng + Send + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;

    type DType: Clone + Send + Sized;
    type Device: Clone + Send + Sized;
    fn dtype(&self) -> Self::DType;
    fn device(&self) -> Self::Device;
}
