//! Brain structure used by this ecosystem's metabolism and observations.

use crate::mind::animal::AnimalMind;

#[derive(Clone, Copy, Debug)]
pub struct BrainSize {
    pub hidden_units: usize,
    /// All stored weights and biases, including zero-valued connections.
    pub weights: usize,
}

/// Controllers without a neural brain report no size and incur no neural upkeep.
/// This experiment-specific contract does not extend the universal Mind interface.
pub trait BrainStructure {
    fn brain_size(&self) -> Option<BrainSize> {
        None
    }
}

impl BrainStructure for AnimalMind {
    fn brain_size(&self) -> Option<BrainSize> {
        Some(BrainSize {
            hidden_units: self.brain.genome.hidden_units,
            weights: self.brain.genome.weights.len(),
        })
    }
}
