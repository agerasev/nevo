#![forbid(unsafe_code)]

mod context;
pub mod foraging;
pub mod herbivore;
pub mod run;

use anyhow::Result;
use rand::Rng;

pub use context::Cpu;

/// Identifies one individual for its entire lifetime in a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentId(pub u64);

/// Simulation context, retaining the draft's RNG and compute metadata contract.
pub trait Context {
    type Rng: Rng + Send + ?Sized;
    fn rng(&mut self) -> &mut Self::Rng;

    type DType: Clone + Send + Sized;
    type Device: Clone + Send + Sized;
    fn dtype(&self) -> Self::DType;
    fn device(&self) -> Self::Device;
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

/// The environment assigns IDs and determines when each individual acts.
pub trait Population<Observation, Action, Origin> {
    fn spawn(&mut self, id: AgentId, origin: Origin);
    fn act(&mut self, id: AgentId, observation: &Observation) -> Action;
    fn despawn(&mut self, id: AgentId);
}

/// An agent's response to one immutable observation of the world.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentResponse<Observation, Action> {
    pub id: AgentId,
    pub observation: Observation,
    pub action: Action,
}

pub trait Environment<P> {
    type Observation;
    type Action;
    type Origin;
    type Report;

    fn observe(&self) -> Vec<(AgentId, Self::Observation)>;

    fn update(
        &mut self,
        population: &mut P,
        responses: Vec<AgentResponse<Self::Observation, Self::Action>>,
    ) -> Self::Report;
}
