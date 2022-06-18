use crate::bacteria::Bacterium;

pub struct Medium {
    bacteria: Vec<Bacterium>,
}

impl Medium {
    pub fn new(bacteria: Vec<Bacterium>) -> Self {
        Self { bacteria }
    }

    pub fn step(&mut self) {}
}
