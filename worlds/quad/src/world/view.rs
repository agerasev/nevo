use std::{
    mem,
    sync::{Arc, RwLock, Weak},
};

use eframe::egui::{
    Color32, ColorImage, Image, Response, TextureFilter, TextureHandle, TextureOptions, Ui, Widget,
};
use glam::UVec2;

use super::{AnimalModel, Block, Plant, World, WorldModel};

pub struct WorldView {
    model: Weak<RwLock<WorldModel>>,
    texture: Option<TextureHandle>,
    last_hash: u64,
}

impl WorldView {
    pub fn new(world: &World) -> Self {
        Self {
            model: Arc::downgrade(&world.model),
            texture: None,
            last_hash: u64::MAX,
        }
    }
}

pub trait Colored {
    fn color(&self) -> Color32;
}

impl Colored for Block {
    fn color(&self) -> Color32 {
        match self {
            Self::Ground => Color32::from_rgb(0, 63, 0),
            Self::Water => Color32::from_rgb(0, 0, 63),
        }
    }
}

impl Colored for Plant {
    fn color(&self) -> Color32 {
        Color32::from_rgb(0, 127, 0)
    }
}

impl Colored for AnimalModel {
    fn color(&self) -> Color32 {
        Color32::from_rgb(255, 0, 0)
    }
}

impl WorldModel {
    pub fn bitmap(&self) -> ColorImage {
        let mut data: Vec<Color32> = self.blocks.items().iter().map(Block::color).collect();
        let mut put_color = |pos: UVec2, color: Color32| {
            let offset = pos.y as usize * self.blocks.size().x as usize + pos.x as usize;
            data[offset] = color;
        };
        for plant in &self.plants {
            put_color(plant.pos, plant.color())
        }
        for animal in &self.animals {
            put_color(animal.pos, animal.color())
        }
        ColorImage {
            size: self.blocks.size().to_array().map(|x| x as usize),
            pixels: data,
        }
    }
}

impl WorldView {
    pub fn draw(&mut self, ui: &mut Ui) -> Response {
        if let Some(model) = self.model.upgrade() {
            let model = model.read().unwrap();
            if mem::replace(&mut self.last_hash, model.hash()) != self.last_hash {
                let image = model.bitmap();
                self.texture = Some(ui.ctx().load_texture(
                    "Map",
                    image,
                    TextureOptions {
                        magnification: TextureFilter::Nearest,
                        ..Default::default()
                    },
                ));
            }
        }
        match &self.texture {
            Some(texture) => Image::new(texture).shrink_to_fit().ui(ui),
            None => ui.label("No map"),
        }
    }
}
