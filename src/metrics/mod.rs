//! Typed, composable metrics. Accumulators remain local to their owners.

mod aggregate;
mod selection;

pub use aggregate::{Aggregate, Count, Max, Mean, Min, Moments, Sum};
pub use selection::{SelectedStats, Selection, Statistics};

/// Read metrics without changing simulation state or its caches.
pub trait Metrics {
    type Summary: Default;

    fn collect_metrics(&self, summary: &mut Self::Summary);

    fn metrics(&self) -> Self::Summary {
        let mut summary = Self::Summary::default();
        self.collect_metrics(&mut summary);
        summary
    }
}

#[cfg(test)]
mod tests;
