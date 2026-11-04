use rand::Rng;

use crate::Context;

/// Native CPU context. Scalar types are fixed by Rust, with no runtime metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Cpu<R> {
    pub rng: R,
}

impl<R: Rng + Send> Context for Cpu<R> {
    type Rng = R;

    fn rng(&mut self) -> &mut Self::Rng {
        &mut self.rng
    }

    type DType = ();
    type Device = ();

    fn dtype(&self) -> Self::DType {}

    fn device(&self) -> Self::Device {}
}
