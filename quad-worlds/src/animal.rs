use eframe::{
    egui::{Pos2, Rgba},
    emath, epaint,
};
use nevo::{
    core::{rand::Rng, Agent, Variable},
    nn::{self, recurrent::Recurrent, Layer},
};
use vecmat::vector::Vector2;

#[derive(Clone, Default, Debug)]
pub struct Input {
    pub food: Vector2<f64>,
    pub neighbor: Vector2<f64>,
    pub obesity: f64,
}

impl Input {
    const SIZE: usize = 5;

    pub fn pack(self) -> nn::Vector {
        let mut dst = nn::Vector::zeros(Self::SIZE);
        (dst.data[0], dst.data[1]) = self.food.into();
        (dst.data[2], dst.data[3]) = self.neighbor.into();
        dst.data[4] = self.obesity;
        dst
    }
}

pub struct Output {
    pub movement: Vector2<f64>,
}

impl Output {
    const SIZE: usize = 2;

    pub fn unpack(src: nn::Vector) -> Self {
        assert_eq!(Self::SIZE, src.len());
        Self {
            movement: Vector2::from([src.data[0], src.data[1]]),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Genome {
    pub network: Recurrent,
}

impl Genome {
    pub fn new<R: Rng>(mem_size: usize, rand_mag: f64, rng: &mut R) -> Self {
        let mut network = Recurrent::new(mem_size, Input::SIZE, Output::SIZE);
        network.variate(
            &nn::recurrent::Rate {
                variance: rand_mag,
                resize: nn::ResizeRate { add_or_remove: 0.0 },
            },
            rng,
        );
        Self { network }
    }
}

pub struct Animal {
    genome: Genome,
    network: Recurrent,
    memory: Option<nn::Vector>,
    position: Vector2<f64>,
}

impl Animal {
    pub fn new(genome: Genome, position: Vector2<f64>) -> Self {
        Self {
            network: genome.network.clone(),
            memory: Some(genome.network.initial_memory().clone()),
            genome,
            position,
        }
    }
}

impl Agent for Animal {
    type Input = Input;
    type Output = Output;

    fn process(&mut self, input: Input) -> Output {
        let (new_mem, out_vec) = self
            .network
            .process((input.pack(), self.memory.take().unwrap()));
        assert!(self.memory.replace(new_mem).is_none());
        Output::unpack(out_vec)
    }
}

impl Animal {
    pub fn draw_shapes(&self, shapes: &mut Vec<epaint::Shape>, transform: emath::RectTransform) {
        let center = {
            let p = self.position.map(|x| x as f32);
            Pos2::new(p.x(), p.y())
        };
        shapes.push(epaint::Shape::circle_filled(
            transform * center,
            transform.scale().length(),
            Rgba::from_rgb(1.0, 0.5, 0.0),
        ));
    }
}
