pub mod init;

use std::collections::HashMap;

use anyhow::Result;
use candle::Tensor;
use init::rand_normal_f64;

use crate::{Context, Genome, Mutate};

impl<C: Context + ?Sized> Genome<C> for Tensor {}

impl<C: Context + ?Sized> Mutate<C, f64> for Tensor {
    fn mutate(&mut self, rate: &f64, cx: &mut C) -> Result<()> {
        *self = (&*self
            + rand_normal_f64(
                cx.rng(),
                self.shape(),
                self.dtype(),
                self.device(),
                0.0,
                *rate,
            )?)?
        .detach();
        Ok(())
    }
}

impl<C: Context + ?Sized, K: Clone> Genome<C> for HashMap<K, Tensor> {}

impl<C: Context + ?Sized, K: Clone> Mutate<C, f64> for HashMap<K, Tensor> {
    fn mutate(&mut self, rate: &f64, cx: &mut C) -> Result<()> {
        for var in self.values_mut() {
            var.mutate(rate, cx)?;
        }
        Ok(())
    }
}
