pub mod init;

use std::collections::HashMap;

use crate::{Genome, Mutate};
use anyhow::Result;
use candle::Tensor;
use init::rand_normal_f64;
use rand::Rng;

impl Genome for Tensor {}

impl Mutate<f64> for Tensor {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: &f64, rng: &mut R) -> Result<()> {
        *self = (&*self
            + rand_normal_f64(rng, self.shape(), self.dtype(), self.device(), 0.0, *rate)?)?
        .detach();
        Ok(())
    }
}

impl<K: Clone> Genome for HashMap<K, Tensor> {}

impl<K: Clone> Mutate<f64> for HashMap<K, Tensor> {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: &f64, rng: &mut R) -> Result<()> {
        for var in self.values_mut() {
            var.mutate(rate, rng)?;
        }
        Ok(())
    }
}
