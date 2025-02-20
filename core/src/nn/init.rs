use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use candle::{DType, Device, Error, Result, Shape, Tensor, Var, WithDType};
use candle_nn::{
    init::{Init, NormalOrUniform},
    var_builder::SimpleBackend,
    VarMap,
};
use half::{bf16, f16};
use rand::Rng;
use rand_distr::{Distribution, Normal, Uniform};

pub fn rand<T: WithDType, R: Rng + ?Sized, D: Distribution<T>>(
    rng: &mut R,
    shape: &Shape,
    device: &Device,
    distr: D,
) -> Result<Tensor> {
    Tensor::from_iter(rng.sample_iter(distr).take(shape.elem_count()), device)?.reshape(shape)
}

pub fn rand_uniform_f64<R: Rng + ?Sized>(
    rng: &mut R,
    shape: &Shape,
    dtype: DType,
    device: &Device,
    min: f64,
    max: f64,
) -> Result<Tensor> {
    match dtype {
        DType::U8 | DType::U32 | DType::I64 => {
            Err(Error::UnsupportedDTypeForOp(dtype, "rand_uniform").bt())
        }
        DType::BF16 => rand(
            rng,
            shape,
            device,
            Uniform::new(bf16::from_f64(min), bf16::from_f64(max)),
        ),
        DType::F16 => rand(
            rng,
            shape,
            device,
            Uniform::new(f16::from_f64(min), f16::from_f64(max)),
        ),
        DType::F32 => rand(rng, shape, device, Uniform::new(min as f32, max as f32)),
        DType::F64 => rand(rng, shape, device, Uniform::new(min, max)),
    }
}

pub fn rand_normal_f64<R: Rng + ?Sized>(
    rng: &mut R,
    shape: &Shape,
    dtype: DType,
    device: &Device,
    mean: f64,
    std: f64,
) -> Result<Tensor> {
    match dtype {
        DType::U8 | DType::U32 | DType::I64 => {
            Err(Error::UnsupportedDTypeForOp(dtype, "rand_uniform").bt())
        }
        DType::BF16 => rand(
            rng,
            shape,
            device,
            Normal::new(bf16::from_f64(mean), bf16::from_f64(std)).map_err(Error::wrap)?,
        ),
        DType::F16 => rand(
            rng,
            shape,
            device,
            Normal::new(f16::from_f64(mean), f16::from_f64(std)).map_err(Error::wrap)?,
        ),
        DType::F32 => rand(
            rng,
            shape,
            device,
            Normal::new(mean as f32, std as f32).map_err(Error::wrap)?,
        ),
        DType::F64 => rand(
            rng,
            shape,
            device,
            Normal::new(mean, std).map_err(Error::wrap)?,
        ),
    }
}

fn init_var<R: Rng + ?Sized, S: Into<Shape>>(
    rng: &mut R,
    init: Init,
    s: S,
    dtype: DType,
    device: &Device,
) -> Result<Var> {
    let shape = s.into();
    Var::from_tensor(&match init {
        Init::Const(0.0) => Tensor::zeros(shape, dtype, device)?,
        Init::Const(1.0) => Tensor::ones(shape, dtype, device)?,
        Init::Const(v) => Tensor::ones(shape, dtype, device)?.affine(v, 0.0)?,
        Init::Uniform { lo, up } => rand_uniform_f64(rng, &shape, dtype, device, lo, up)?,
        Init::Randn { mean, stdev } => rand_normal_f64(rng, &shape, dtype, device, mean, stdev)?,
        Init::Kaiming {
            dist,
            fan,
            non_linearity,
        } => {
            let fan = fan.for_shape(&shape);
            let gain = non_linearity.gain();
            let std = gain / (fan as f64).sqrt();
            match dist {
                NormalOrUniform::Uniform => {
                    let bound = 3f64.sqrt() * std;
                    rand_uniform_f64(rng, &shape, dtype, device, -bound, bound)?
                }
                NormalOrUniform::Normal => rand_normal_f64(rng, &shape, dtype, device, 0.0, std)?,
            }
        }
    })
}

/// Determined [`VarMap`]
#[derive(Clone)]
pub struct DetermVarMap<R: Rng + Send> {
    inner: VarMap,
    rng: Arc<Mutex<R>>,
}

impl<R: Rng + Send> DetermVarMap<R> {
    pub fn new(inner: VarMap, rng: R) -> Self {
        Self {
            inner,
            rng: Arc::new(Mutex::new(rng)),
        }
    }
    pub fn into_inner(self) -> VarMap {
        self.inner
    }
}

impl<R: Rng + Send> Deref for DetermVarMap<R> {
    type Target = VarMap;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<R: Rng + Send> DerefMut for DetermVarMap<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<R: Rng + Send> SimpleBackend for DetermVarMap<R> {
    fn get(
        &self,
        shape: Shape,
        name: &str,
        init: Init,
        dtype: DType,
        device: &Device,
    ) -> Result<Tensor> {
        if !self.contains_tensor(name) {
            let mut data = self.inner.data().lock().unwrap();
            let mut rng = self.rng.try_lock().unwrap();
            assert!(data
                .insert(
                    name.to_string(),
                    init_var(rng.deref_mut(), init, &shape, dtype, device)?,
                )
                .is_none());
        }
        self.inner.get(shape, name, init, dtype, device)
    }
    fn contains_tensor(&self, name: &str) -> bool {
        self.inner.contains_tensor(name)
    }
}
