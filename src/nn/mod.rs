pub mod init;

use std::collections::HashMap;

use crate::Genome;
use anyhow::Result;
use candle::Var;
use init::rand_normal_f64;
use rand::Rng;

impl Genome for Var {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: f64, rng: &mut R) -> Result<()> {
        self.set(
            &(self.as_detached_tensor()
                + rand_normal_f64(rng, self.shape(), self.dtype(), self.device(), 0.0, rate))?,
        )?;
        Ok(())
    }
}

impl<K: Clone> Genome for HashMap<K, Var> {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: f64, rng: &mut R) -> Result<()> {
        for var in self.values_mut() {
            var.mutate(rate, rng)?;
        }
        Ok(())
    }
}
