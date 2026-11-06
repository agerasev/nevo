//! Execution context shared by agents, minds, and genome operations.

mod cpu;

use rand::Rng;

pub use cpu::Cpu;

/// Randomness supplied to mind and genome operations.
pub trait Context {
    type Rng: Rng + Send + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;
}
