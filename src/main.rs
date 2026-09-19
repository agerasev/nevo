#![forbid(unsafe_code)]

use glam::Vec2;
use rand::{Rng, RngExt, SeedableRng, distr::Uniform, rngs::SmallRng};
use wgame::{
    Library, Result, Window,
    gfx::{
        Scene,
        types::{Color, color},
    },
    prelude::*,
};

pub struct WorldConfig {
    pub size: Vec2,
    pub n_plants: usize,
    pub n_animals: usize,
    pub max_plant_mass: f32,
    pub max_animal_mass: f32,
}

pub struct World {
    config: WorldConfig,
    plants: Vec<Plant>,
    animals: Vec<Animal>,
}

pub struct Animal {
    pub pos: Vec2,
    pub mass: f32,
}

pub struct Plant {
    pub pos: Vec2,
    pub mass: f32,
}

pub trait Drawable {
    fn draw(&self, lib: &Library, scene: &mut Scene);
}

const BORDER_WIDTH: f32 = 0.1;

impl Drawable for Animal {
    fn draw(&self, lib: &Library, scene: &mut Scene) {
        scene.add(
            &lib.shapes()
                .unit_circle()
                .scale(self.mass.sqrt())
                .move_to(self.pos)
                .fill_color(color::RED.mix(color::YELLOW, 0.25))
                .order(3),
        );
        scene.add(
            &lib.shapes()
                .unit_circle()
                .scale(self.mass.sqrt() + BORDER_WIDTH)
                .move_to(self.pos)
                .fill_color(color::BLACK)
                .order(2),
        );
    }
}

impl Drawable for Plant {
    fn draw(&self, lib: &Library, scene: &mut Scene) {
        scene.add(
            &lib.shapes()
                .unit_circle()
                .scale(self.mass.sqrt())
                .move_to(self.pos)
                .fill_color(color::GREEN)
                .order(1),
        );
        scene.add(
            &lib.shapes()
                .unit_circle()
                .scale(self.mass.sqrt() + BORDER_WIDTH)
                .move_to(self.pos)
                .fill_color(color::BLACK)
                .order(0),
        );
    }
}

impl Drawable for World {
    fn draw(&self, lib: &Library, scene: &mut Scene) {
        scene.add(
            &lib.shapes()
                .rectangle((Vec2::ZERO, self.config.size))
                .fill_color(color::GREEN.mix(color::BLACK, 0.75))
                .order(-1),
        );

        for plant in &self.plants {
            plant.draw(lib, scene);
        }
        for animal in &self.animals {
            animal.draw(lib, scene);
        }
    }
}

impl World {
    pub fn new(config: WorldConfig, rng: &mut impl Rng) -> Self {
        Self {
            plants: (0..config.n_plants)
                .map(|_| Plant {
                    pos: rng.sample(Uniform::new(Vec2::ZERO, config.size).unwrap()),
                    mass: rng.sample(Uniform::new(0.0, config.max_plant_mass).unwrap()),
                })
                .collect(),
            animals: (0..config.n_animals)
                .map(|_| Animal {
                    pos: rng.sample(Uniform::new(Vec2::ZERO, config.size).unwrap()),
                    mass: rng.sample(Uniform::new(0.0, config.max_animal_mass).unwrap()),
                })
                .collect(),
            config,
        }
    }
}

#[wgame::window(title = "NEvo", size = (1200, 900), resizable = true, vsync = true)]
async fn main(mut window: Window<'_>) -> Result<()> {
    let mut rng = SmallRng::seed_from_u64(0xdeadbeef);
    let world = World::new(
        WorldConfig {
            size: Vec2::new(100.0, 100.0),
            n_plants: 1000,
            n_animals: 100,
            max_plant_mass: 3.0,
            max_animal_mass: 1.0,
        },
        &mut rng,
    );

    let lib = Library::new(window.graphics());
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    let mut frames = 0;

    while let Some(mut frame) = window.next_frame().await? {
        frame.clear(color::BLACK);

        let mut scene = frame.scene();
        scene.camera = scene.camera.scale(0.018).move_to(-world.config.size / 2.0);
        world.draw(&lib, &mut scene);
        scene.render();
        frame.present();

        frames += 1;
        if smoke && frames == 12 {
            break;
        }
    }

    Ok(())
}
