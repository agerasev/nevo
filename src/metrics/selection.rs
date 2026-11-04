use super::{Aggregate, Count, Max, Mean, Min, Moments, Sum};

/// Requested reductions for one scalar metric. Every reduction defaults off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub count: bool,
    pub sum: bool,
    pub mean: bool,
    pub min: bool,
    pub max: bool,
    pub variance: bool,
}

impl Selection {
    pub const fn enabled(self) -> bool {
        self.count || self.sum || self.mean || self.min || self.max || self.variance
    }

    pub const fn all() -> Self {
        Self {
            count: true,
            sum: true,
            mean: true,
            min: true,
            max: true,
            variance: true,
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            count: self.count || other.count,
            sum: self.sum || other.sum,
            mean: self.mean || other.mean,
            min: self.min || other.min,
            max: self.max || other.max,
            variance: self.variance || other.variance,
        }
    }
}

/// Final values, suitable for output. Empty samples have no extrema or moments.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Statistics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variance: Option<f64>,
}

/// Only requested reducers are updated. This cache is deliberately not serializable.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelectedStats {
    selection: Selection,
    count: Count,
    sum: Sum,
    mean: Mean,
    min: Min,
    max: Max,
    moments: Moments,
}

impl SelectedStats {
    pub fn new(selection: Selection) -> Self {
        Self {
            selection,
            ..Self::default()
        }
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn enabled(&self) -> bool {
        self.selection.enabled()
    }

    pub fn push(&mut self, value: f64) {
        if self.selection.count {
            self.count.push(());
        }
        if self.selection.sum {
            self.sum.push(value);
        }
        if self.selection.mean {
            self.mean.push(value);
        }
        if self.selection.min {
            self.min.push(value);
        }
        if self.selection.max {
            self.max.push(value);
        }
        if self.selection.variance {
            self.moments.push(value);
        }
    }

    /// Each reduction combines the samples for which it was enabled. This also
    /// permits an empty default summary to adopt the incoming configuration.
    pub fn merge(&mut self, other: &Self) {
        self.selection = self.selection.union(other.selection);
        self.count.merge(&other.count);
        self.sum.merge(&other.sum);
        self.mean.merge(&other.mean);
        self.min.merge(&other.min);
        self.max.merge(&other.max);
        self.moments.merge(&other.moments);
    }

    pub fn value(&self) -> Statistics {
        Statistics {
            count: self.selection.count.then(|| self.count.value()),
            sum: self.selection.sum.then(|| self.sum.value()),
            mean: self.selection.mean.then(|| self.mean.value()).flatten(),
            min: self.selection.min.then(|| self.min.value()).flatten(),
            max: self.selection.max.then(|| self.max.value()).flatten(),
            variance: self
                .selection
                .variance
                .then(|| self.moments.variance())
                .flatten(),
        }
    }
}

impl Aggregate<f64> for SelectedStats {
    type Output = Statistics;

    fn push(&mut self, value: f64) {
        Self::push(self, value);
    }
    fn merge(&mut self, other: &Self) {
        Self::merge(self, other);
    }
    fn value(&self) -> Statistics {
        Self::value(self)
    }
}
