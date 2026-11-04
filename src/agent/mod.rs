//! Agent processing and population lifecycle contracts, with concrete adapters.

pub mod herbivore;

use anyhow::Result;

use crate::context::Context;

/// Identifies one individual for its entire lifetime in a world.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct AgentId(pub u64);

/// Stateful agent.
pub trait Agent<C: Context + ?Sized> {
    type Input;
    type Output;

    /// Perform single step of processing.
    fn process(&mut self, cx: &mut C, input: Self::Input) -> Result<Self::Output>;
}

/// The environment assigns IDs and determines when each individual acts.
pub trait Population<Observation, Action, Origin> {
    fn spawn(&mut self, id: AgentId, origin: Origin);
    fn act(&mut self, id: AgentId, observation: &Observation) -> Action;
    fn despawn(&mut self, id: AgentId);
}
