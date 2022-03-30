#![forbid(unsafe_code)]

pub trait Agent {
    type Input;
    type Output;

    fn process(&mut self, input: Self::Input) -> Self::Output;
}

pub trait System {
    fn process(&mut self);
}
