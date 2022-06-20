pub use rand;

use rand::Rng;

pub trait Variable {
    type Rate;

    fn variate<R: Rng>(&mut self, rate: &Self::Rate, rng: &mut R);

    /// Number of degrees of freedom.
    fn dof(&self) -> usize;
}

pub trait Agent {
    type Input;
    type Output;

    fn process(&mut self, input: Self::Input) -> Self::Output;
}

pub trait System {
    fn process(&mut self);
}
