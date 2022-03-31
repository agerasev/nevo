use ndarray::Array1;
use nevo::core::Agent;
use vecmat::Vector;

pub struct Input {
    food: Vector<f64, 2>,
    neighbor: Vector<f64, 2>,
    obesity: f64,
}

pub struct Output {
    movement: Vector<f64, 2>,
}

pub struct Bacteria {
    memory: Array1<f64>,
}

// impl Agent for Bacteria {}
