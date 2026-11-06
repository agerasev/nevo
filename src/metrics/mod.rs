//! Read-only summaries; experiment-specific observations stay in the ecosystem.

pub mod log;

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
