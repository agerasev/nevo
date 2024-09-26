pub use ndarray;

pub mod recurrent;

use ndarray::{Array0, Array1, Array2, Axis};
use nevo_core::{rand::Rng, Variable};
use rand_distr::{Bernoulli, Normal, Uniform};

#[derive(Clone, Copy, Debug)]
pub enum ResizeAction {
    Add,
    Remove(usize),
}

#[derive(Clone, Debug)]
pub struct ResizeRate {
    pub add_or_remove: f64,
}

impl ResizeRate {
    pub fn sample<R: Rng>(&self, rng: &mut R, size: usize) -> Option<ResizeAction> {
        if rng.sample(Bernoulli::new(self.add_or_remove).unwrap()) {
            Some(if rng.sample(Bernoulli::new(0.5).unwrap()) {
                ResizeAction::Add
            } else {
                ResizeAction::Remove(rng.sample(Uniform::new(0, size)))
            })
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct Vector {
    pub data: Array1<f64>,
}

impl Vector {
    pub fn new(data: Array1<f64>) -> Self {
        Self { data }
    }

    pub fn zeros(len: usize) -> Self {
        Self {
            data: Array1::zeros(len),
        }
    }

    pub fn len(&self) -> usize {
        let shape = self.data.shape();
        debug_assert_eq!(shape.len(), 1);
        shape[0]
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn resize(&mut self, action: ResizeAction) {
        match action {
            ResizeAction::Add => self.data.push(Axis(0), Array0::zeros(()).view()).unwrap(),
            ResizeAction::Remove(i) => self.data.remove_index(Axis(0), i),
        }
    }

    pub fn add(&self, other: &Vector) -> Self {
        Vector::new(&self.data + &other.data)
    }
}

impl Variable for Vector {
    type Rate = f64;
    fn variate<R: Rng>(&mut self, rate: &f64, rng: &mut R) {
        self.data
            .mapv_inplace(|x| x + rng.sample(Normal::new(0.0, *rate).unwrap()));
    }

    fn dof(&self) -> usize {
        self.data.shape()[0]
    }
}

pub trait Layer {
    type Input;
    type Output;

    fn process(&self, input: Self::Input) -> Self::Output;
}

pub trait Apply<O, L: Layer<Input = Self, Output = O>>: Sized {
    fn apply(self, layer: &L) -> O {
        layer.process(self)
    }
}

impl<O, L: Layer<Input = Self, Output = O>> Apply<O, L> for Vector {}

#[derive(Clone, Debug)]
pub struct Linear {
    pub data: Array2<f64>,
}

impl Linear {
    pub fn new(input_size: usize, output_size: usize) -> Self {
        Self {
            data: Array2::zeros((input_size, output_size)),
        }
    }

    pub fn sizes(&self) -> (usize, usize) {
        let shape = self.data.shape();
        (shape[0], shape[1])
    }

    fn resize(&mut self, index: bool, action: ResizeAction) {
        let axis = Axis(index as usize);
        match action {
            ResizeAction::Add => {
                let zeros = Array1::zeros(self.data.shape()[(!index) as usize]);
                self.data.push(axis, zeros.view()).unwrap();
            }
            ResizeAction::Remove(i) => self.data.remove_index(axis, i),
        }
    }

    pub fn resize_input(&mut self, action: ResizeAction) {
        self.resize(false, action);
    }
    pub fn resize_output(&mut self, action: ResizeAction) {
        self.resize(true, action);
    }
}

impl Layer for Linear {
    type Input = Vector;
    type Output = Vector;

    fn process(&self, input: Vector) -> Vector {
        Vector::new(self.data.dot(&input.data))
    }
}

impl Variable for Linear {
    type Rate = f64;

    fn variate<R: Rng>(&mut self, rate: &f64, rng: &mut R) {
        let distr = Normal::new(0.0, *rate).unwrap();
        self.data.mapv_inplace(|x| x + rng.sample(distr));
    }

    fn dof(&self) -> usize {
        let (input_size, output_size) = self.sizes();
        input_size * output_size
    }
}

#[derive(Clone, Debug)]
pub struct Bias {
    pub data: Array1<f64>,
}

impl Bias {
    pub fn new(size: usize) -> Self {
        Self {
            data: Array1::zeros(size),
        }
    }

    pub fn size(&self) -> usize {
        self.data.shape()[0]
    }

    fn resize(&mut self, action: ResizeAction) {
        match action {
            ResizeAction::Add => self.data.push(Axis(0), Array0::zeros(()).view()).unwrap(),
            ResizeAction::Remove(i) => self.data.remove_index(Axis(0), i),
        }
    }
}

impl Layer for Bias {
    type Input = Vector;
    type Output = Vector;

    fn process(&self, mut input: Vector) -> Vector {
        input.data += &self.data;
        input
    }
}

impl Variable for Bias {
    type Rate = f64;

    fn variate<R: Rng>(&mut self, rate: &f64, rng: &mut R) {
        let distr = Normal::new(0.0, *rate).unwrap();
        self.data.mapv_inplace(|x| x + rng.sample(distr));
    }

    fn dof(&self) -> usize {
        self.size()
    }
}

pub struct Tanh;

impl Tanh {
    fn process_inplace(vector: &mut Vector) {
        vector.data.mapv_inplace(|x| x.tanh());
    }
}

impl Layer for Tanh {
    type Input = Vector;
    type Output = Vector;

    fn process(&self, mut input: Vector) -> Vector {
        Self::process_inplace(&mut input);
        input
    }
}
