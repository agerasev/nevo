use crate::cell::Cell;

pub struct Medium {
    cells: Vec<Cell>,
}

impl Medium {
    pub fn new(cells: Vec<Cell>) -> Self {
        Self { cells }
    }

    pub fn step(&mut self) {}
}
