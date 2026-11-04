//! Physical state and typed interaction, independent of controller implementations.

pub mod ecosystem;

use anyhow::Result;

use crate::actor::HandleAllocator;

pub trait Environment {
    type Observation;
    type Action;
    type Report;
    type Request;
    type Intervention;

    fn observe(&self) -> Self::Observation;

    /// Advance private state, asking the experiment for external interventions.
    /// The returned observation synchronizes actor state before publication.
    fn advance(
        &mut self,
        tick: u64,
        action: Self::Action,
        handles: &mut HandleAllocator,
        intervene: impl FnMut(Self::Request) -> Result<Self::Intervention>,
    ) -> Result<(Self::Report, Self::Observation)>;

    fn validate(&self, handles: &HandleAllocator) -> Result<()>;
}
