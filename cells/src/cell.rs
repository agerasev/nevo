use nevo::{
    core::Agent,
    nn::{self, recurrent::Recurrent, Layer},
};
use vecmat::Vector;

#[derive(Clone, Default, Debug)]
pub struct Input {
    pub food: Vector<f64, 2>,
    pub neighbor: Vector<f64, 2>,
    pub obesity: f64,
}

impl Input {
    pub fn pack(self) -> nn::Vector {
        let mut dst = nn::Vector::zeros(5);
        (dst.data[0], dst.data[1]) = self.food.into();
        (dst.data[2], dst.data[3]) = self.neighbor.into();
        dst.data[4] = self.obesity;
        dst
    }
}

pub struct Output {
    pub movement: Vector<f64, 2>,
}

impl Output {
    pub fn unpack(src: nn::Vector) -> Self {
        assert_eq!(2, src.len());
        Self {
            movement: Vector::from([src.data[0], src.data[1]]),
        }
    }
}

pub struct Cell {
    memory: Option<nn::Vector>,
    brain: Recurrent,
}

impl Agent for Cell {
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
