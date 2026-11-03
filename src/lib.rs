#![forbid(unsafe_code)]

pub mod foraging;
pub mod herbivore;
pub mod run;

/// Identifies one individual for its entire lifetime in a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentId(pub u64);

pub trait Controller {
    type Observation;
    type Action;

    fn act(&mut self, observation: &Self::Observation) -> Self::Action;
}

/// The environment assigns IDs and determines when each individual acts.
pub trait Population<Observation, Action, Origin> {
    fn spawn(&mut self, id: AgentId, origin: Origin);
    fn act(&mut self, id: AgentId, observation: &Observation) -> Action;
    fn despawn(&mut self, id: AgentId);
}

pub trait Environment<P> {
    type Report;

    fn tick(&mut self, population: &mut P) -> Self::Report;
}
