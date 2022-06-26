use super::*;

#[derive(Clone, Debug)]
pub struct Recurrent {
    initial_memory: Vector,

    enter: Linear,
    enter_bias: Bias,

    exit: Linear,
    exit_bias: Bias,

    recurse: Linear,
}

pub struct Sizes {
    pub input: usize,
    pub output: usize,
    pub memory: usize,
}

trait ForEachVariable {
    type Rate;
    fn call<V: Variable<Rate = Self::Rate>>(&mut self, variable: &V);
}

trait ForEachVariableMut {
    type Rate;
    fn call<V: Variable<Rate = Self::Rate>>(&mut self, variable: &mut V);
}

impl Recurrent {
    pub fn new(memory_size: usize, input_size: usize, output_size: usize) -> Self {
        Self {
            initial_memory: Vector::new(Array1::zeros(memory_size)),
            enter: Linear::new(input_size, memory_size),
            enter_bias: Bias::new(memory_size),
            exit: Linear::new(memory_size, output_size),
            exit_bias: Bias::new(output_size),
            recurse: Linear::new(memory_size, memory_size),
        }
    }

    pub fn sizes(&self) -> Sizes {
        Sizes {
            input: self.enter_bias.size(),
            output: self.exit_bias.size(),
            memory: self.initial_memory.len(),
        }
    }

    pub fn initial_memory(&self) -> &Vector {
        &self.initial_memory
    }

    pub fn resize_input(&mut self, action: ResizeAction) {
        self.enter.resize_input(action);
    }

    pub fn resize_output(&mut self, action: ResizeAction) {
        self.exit.resize_output(action);
        self.exit_bias.resize(action);
    }

    pub fn resize_memory(&mut self, action: ResizeAction) {
        self.initial_memory.resize(action);
        self.enter.resize_output(action);
        self.enter_bias.resize(action);
        self.exit.resize_input(action);
        self.recurse.resize_input(action);
        self.recurse.resize_output(action);
    }

    fn for_each_variable<F: ForEachVariable<Rate = f64>>(&self, f: &mut F) {
        f.call(&self.initial_memory);
        f.call(&self.enter);
        f.call(&self.enter_bias);
        f.call(&self.exit);
        f.call(&self.exit_bias);
        f.call(&self.recurse);
    }

    fn for_each_variable_mut<F: ForEachVariableMut<Rate = f64>>(&mut self, f: &mut F) {
        f.call(&mut self.initial_memory);
        f.call(&mut self.enter);
        f.call(&mut self.enter_bias);
        f.call(&mut self.exit);
        f.call(&mut self.exit_bias);
        f.call(&mut self.recurse);
    }
}

impl Layer for Recurrent {
    type Input = (Vector, Vector);
    type Output = (Vector, Vector);

    fn process(&self, (input, memory): (Vector, Vector)) -> (Vector, Vector) {
        let new_memory = input
            .apply(&self.enter)
            .add(&memory.apply(&self.recurse))
            .apply(&self.enter_bias)
            .apply(&Tanh);

        let output = new_memory
            .clone()
            .apply(&self.exit)
            .apply(&self.exit_bias)
            .apply(&Tanh);

        (output, new_memory)
    }
}

pub struct Rate {
    pub variance: f64,
    pub resize: ResizeRate,
}

struct DofCounter(pub usize);

impl ForEachVariable for DofCounter {
    type Rate = f64;

    fn call<V: Variable<Rate = Self::Rate>>(&mut self, variable: &V) {
        self.0 += variable.dof();
    }
}

struct Variator<'a, R: Rng> {
    rng: &'a mut R,
    rate: f64,
}

impl<'a, R: Rng> ForEachVariableMut for Variator<'a, R> {
    type Rate = f64;

    fn call<V: Variable<Rate = Self::Rate>>(&mut self, variable: &mut V) {
        variable.variate(&self.rate, self.rng);
    }
}

impl Variable for Recurrent {
    type Rate = Rate;
    fn variate<R: Rng>(&mut self, rate: &Rate, rng: &mut R) {
        if let Some(action) = rate.resize.sample(rng, self.sizes().memory) {
            self.resize_memory(action);
        }

        self.for_each_variable_mut(&mut Variator {
            rng,
            rate: rate.variance,
        });
    }

    fn dof(&self) -> usize {
        let mut counter = DofCounter(0);
        self.for_each_variable(&mut counter);
        counter.0
    }
}
