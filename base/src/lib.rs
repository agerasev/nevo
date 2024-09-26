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

pub trait System {
    fn step<C: Context>(&mut self, cx: &mut C);
}
