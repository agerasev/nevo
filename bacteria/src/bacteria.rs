use nevo::core::{
    neural::{self, recurrent::Recurrent, Layer},
    Agent,
};
use vecmat::Vector;

#[derive(Clone, Default, Debug)]
pub struct Input {
    pub food: Vector<f64, 2>,
    pub neighbor: Vector<f64, 2>,
    pub obesity: f64,
}

impl Input {
    pub fn pack(self) -> neural::Vector {
        let mut dst = neural::Vector::zeros(5);
        (dst[0], dst[1]) = self.food.into();
        (dst[2], dst[3]) = self.neighbor.into();
        dst[4] = self.obesity;
        dst
    }
}

pub struct Output {
    pub movement: Vector<f64, 2>,
}

impl Output {
    pub fn unpack(src: neural::Vector) -> Self {
        assert_eq!([2], src.shape());
        Self {
            movement: Vector::from([src[0], src[1]]),
        }
    }
}

pub struct Bacterium {
    memory: Option<neural::Vector>,
    brain: Recurrent,
}

impl Agent for Bacterium {
    type Input = Input;
    type Output = Output;

    fn process(&mut self, input: Input) -> Output {
        let (new_mem, out_vec) = self
            .brain
            .process((input.pack(), self.memory.take().unwrap()));
        assert!(self.memory.replace(new_mem).is_none());
        Output::unpack(out_vec)
    }
}
