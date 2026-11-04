use super::Statistics;

/// An accumulator which can combine independently collected samples.
pub trait Aggregate<T>: Default {
    type Output;

    fn push(&mut self, value: T);
    fn merge(&mut self, other: &Self);
    fn value(&self) -> Self::Output;
}

/// Integer count of observations (including empty observations).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Count(u64);

impl Aggregate<()> for Count {
    type Output = u64;

    fn push(&mut self, (): ()) {
        self.0 += 1;
    }

    fn merge(&mut self, other: &Self) {
        self.0 += other.0;
    }

    fn value(&self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sum(f64);

impl Aggregate<f64> for Sum {
    type Output = f64;

    fn push(&mut self, value: f64) {
        self.0 += value;
    }

    fn merge(&mut self, other: &Self) {
        self.0 += other.0;
    }

    fn value(&self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Min(Option<f64>);

impl Aggregate<f64> for Min {
    type Output = Option<f64>;

    fn push(&mut self, value: f64) {
        self.0 = Some(self.0.map_or(value, |old| old.min(value)));
    }

    fn merge(&mut self, other: &Self) {
        if let Some(value) = other.0 {
            self.push(value);
        }
    }

    fn value(&self) -> Option<f64> {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Max(Option<f64>);

impl Aggregate<f64> for Max {
    type Output = Option<f64>;

    fn push(&mut self, value: f64) {
        self.0 = Some(self.0.map_or(value, |old| old.max(value)));
    }

    fn merge(&mut self, other: &Self) {
        if let Some(value) = other.0 {
            self.push(value);
        }
    }

    fn value(&self) -> Option<f64> {
        self.0
    }
}

/// Incremental mean; does not calculate variance when only a mean is requested.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mean {
    count: u64,
    mean: f64,
}

impl Aggregate<f64> for Mean {
    type Output = Option<f64>;

    fn push(&mut self, value: f64) {
        self.count += 1;
        self.mean += (value - self.mean) / self.count as f64;
    }

    fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        let total = self.count + other.count;
        self.mean += (other.mean - self.mean) * (other.count as f64 / total as f64);
        self.count = total;
    }

    fn value(&self) -> Option<f64> {
        (self.count > 0).then_some(self.mean)
    }
}

/// Welford moments with the parallel merge formula. Variance is population M2/n.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moments {
    count: u64,
    mean: f64,
    m2: f64,
}

impl Moments {
    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then_some(self.mean)
    }

    pub fn variance(&self) -> Option<f64> {
        (self.count > 0).then(|| self.m2 / self.count as f64)
    }
}

impl Aggregate<f64> for Moments {
    type Output = Statistics;

    fn push(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (value - self.mean);
    }

    fn merge(&mut self, other: &Self) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            *self = *other;
            return;
        }
        let total = self.count + other.count;
        let delta = other.mean - self.mean;
        self.m2 +=
            other.m2 + delta * delta * (self.count as f64 * (other.count as f64 / total as f64));
        self.mean += delta * (other.count as f64 / total as f64);
        self.count = total;
    }

    fn value(&self) -> Statistics {
        Statistics {
            count: Some(self.count),
            mean: self.mean(),
            variance: self.variance(),
            ..Statistics::default()
        }
    }
}
