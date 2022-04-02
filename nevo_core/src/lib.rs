pub mod neural;

use rand::Rng;

pub trait Variable {
    type Rate;
    fn variate<R: Rng>(&mut self, rate: &Self::Rate, rng: &mut R);
}

pub trait Agent {
    type Input;
    type Output;

    fn process(&mut self, input: &Self::Input, output: &mut Self::Output);
}

pub trait System {
    fn process(&mut self);
}
