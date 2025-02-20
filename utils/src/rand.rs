use glam::UVec2;
use rand::{
    distributions::{Distribution, Uniform},
    Rng,
};

pub struct AxisAlignedUniform<T>(pub T, pub T);

impl Distribution<UVec2> for AxisAlignedUniform<UVec2> {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> UVec2 {
        UVec2::from([
            rng.sample(Uniform::new(self.0.x, self.1.x)),
            rng.sample(Uniform::new(self.0.y, self.1.y)),
        ])
    }
}
