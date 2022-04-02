use crate::Variable;
use ndarray::{linalg::general_mat_vec_mul, Array1, Array2};
use rand::Rng;
use rand_distr::Normal;
use std::cell::RefCell;

pub type Vector = Array1<f64>;

pub trait Pack {
    fn pack(&self, dst: &mut Vector);
}

pub trait Unpack {
    fn unpack(&mut self, src: &Vector);
}

pub trait Layer {
    fn process(&self, input: &Vector, output: &mut Vector);
}

impl Variable for Vector {
    type Rate = f64;
    fn variate<R: Rng>(&mut self, rate: &f64, rng: &mut R) {
        self.mapv_inplace(|x| x + rng.sample(Normal::new(0.0, *rate).unwrap()));
    }
}

/// Fully-connected layer with bias.
#[derive(Clone, Debug)]
pub struct Fc {
    pub weight: Array2<f64>,
    pub bias: Array1<f64>,
}

impl Fc {
    pub fn new(input: usize, output: usize) -> Self {
        Self {
            weight: Array2::zeros((input, output)),
            bias: Array1::zeros(output),
        }
    }
}

impl Layer for Fc {
    fn process(&self, input: &Array1<f64>, output: &mut Array1<f64>) {
        output.assign(&self.bias);
        general_mat_vec_mul(1.0, &self.weight, input, 1.0, output);
    }
}

impl Variable for Fc {
    type Rate = f64;
    fn variate<R: Rng>(&mut self, rate: &f64, rng: &mut R) {
        let distr = Normal::new(0.0, *rate).unwrap();
        self.weight.mapv_inplace(|x| x + rng.sample(distr));
        self.bias.mapv_inplace(|x| x + rng.sample(distr));
    }
}

/// Element-wise hyperbolic tangent function.
pub struct Tanh;

impl Tanh {
    fn process_inplace(data: &mut Array1<f64>) {
        data.mapv_inplace(|x| x.tanh());
    }
}

impl Layer for Tanh {
    fn process(&self, input: &Array1<f64>, output: &mut Array1<f64>) {
        output.assign(input);
        Self::process_inplace(output);
    }
}

/// Naive recursive cell.
#[derive(Clone, Debug)]
pub struct Rnn {
    pub enter: Fc,
    pub exit: Fc,
    pub recurse: Fc,
    memory_cache: RefCell<Array1<f64>>,
}

impl Rnn {
    pub fn new(memory: usize, input: usize, output: usize) -> Self {
        Self {
            enter: Fc::new(input, memory),
            exit: Fc::new(memory, output),
            recurse: Fc::new(memory, memory),
            memory_cache: RefCell::new(Array1::zeros(memory)),
        }
    }

    pub fn process(&self, memory: &mut Array1<f64>, input: &Array1<f64>, output: &mut Array1<f64>) {
        let mut cache = self.memory_cache.borrow_mut();

        cache.assign(memory);
        self.recurse.process(&*cache, memory);
        Tanh::process_inplace(memory);

        self.enter.process(input, &mut *cache);
        *memory += &*cache;

        self.exit.process(memory, output);
    }
}

impl Variable for Rnn {
    type Rate = f64;
    fn variate<R: Rng>(&mut self, rate: &f64, rng: &mut R) {
        self.enter.variate(rate, rng);
        self.exit.variate(rate, rng);
        self.recurse.variate(rate, rng);
    }
}
