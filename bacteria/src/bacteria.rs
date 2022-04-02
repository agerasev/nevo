use ndarray::Array1;
use nevo::core::{
    neural::{self, Pack as _, Rnn, Unpack as _},
    Agent,
};
use vecmat::Vector;

#[derive(Clone, Default, Debug)]
pub struct Input {
    food: Vector<f64, 2>,
    neighbor: Vector<f64, 2>,
    obesity: f64,
}

impl neural::Pack for Input {
    fn pack(&self, dst: &mut neural::Vector) {
        assert_eq!([5], dst.shape());
        (dst[0], dst[1]) = self.food.into();
        (dst[2], dst[3]) = self.neighbor.into();
        dst[4] = self.obesity;
    }
}

pub struct Output {
    movement: Vector<f64, 2>,
}

impl neural::Unpack for Output {
    fn unpack(&mut self, src: &neural::Vector) {
        assert_eq!([2], src.shape());
        self.movement = Vector::from([src[0], src[1]]);
    }
}

#[derive(Default, Debug)]
pub struct Cache {
    in_vec: Array1<f64>,
    out_vec: Array1<f64>,
}

pub struct Bacteria {
    memory: Array1<f64>,
    brain: Rnn,
    cache: Cache,
}

impl Agent for Bacteria {
    type Input = Input;
    type Output = Output;

    fn process(&mut self, input: &Input, output: &mut Output) {
        input.pack(&mut self.cache.in_vec);
        self.brain.process(
            &mut self.memory,
            &self.cache.in_vec,
            &mut self.cache.out_vec,
        );
        output.unpack(&self.cache.out_vec);
    }
}
