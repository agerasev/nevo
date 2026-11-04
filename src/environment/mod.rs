//! World observation and update contracts, independent of body and controller types.

pub mod ecosystem;

use crate::agent::AgentId;

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
