use rand::Rng;

/// Simulation context.
pub trait Context {
    type Rng: Rng + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;
}

/// Stateful agent.
pub trait Agent {
    type Input;
    type Output;

    /// Perform single step of processing.
    fn process<C: Context>(&mut self, cx: &mut C, input: Self::Input) -> Self::Output;
}

pub trait Genome: Clone + Sized {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: f64, rng: &mut R);
}

pub trait Sexual: Genome {
    fn recombine<R: Rng + ?Sized>(&self, other: &Self, rng: &mut R) -> Option<Self>;
}

pub trait Evolving {
    type Genome: Genome;
    fn genome(&self) -> Self::Genome;
    fn instance<C: Context>(genome: &Self::Genome, cx: &mut C) -> Self;
}

pub trait System {
    fn step<C: Context>(&mut self, cx: &mut C);
}
