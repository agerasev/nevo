use glam::Vec2;
use wgame::{
    Library,
    gfx::{Scene, Target},
    prelude::*,
    rgb::Rgb,
};

use super::Camera;

/// World-space region to fit into a viewport, preserving its aspect ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

/// Display text, optionally colored by the object that supplies it.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    pub color: Option<Rgb<f32>>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
        }
    }

    pub fn colored(text: impl Into<String>, color: Rgb<f32>) -> Self {
        Self {
            text: text.into(),
            color: Some(color),
        }
    }
}

/// An owned wgame scene and its display metadata, borrowing only reusable graphics
/// resources. No simulation state is retained. Build under a read lock, then
/// release the lock before rendering or presenting the frame.
pub struct Drawing<'a> {
    pub library: &'a Library,
    pub scene: Scene,
    pub bounds: Option<Bounds>,
    pub background: Rgb<f32>,
    pub labels: Vec<Label>,
}

impl<'a> Drawing<'a> {
    pub fn new(library: &'a Library) -> Self {
        Self {
            library,
            scene: Scene::default(),
            bounds: None,
            background: Rgb::new(0.055, 0.07, 0.075),
            labels: Vec::new(),
        }
    }

    pub fn include_bounds(&mut self, bounds: Bounds) {
        self.bounds = Some(match self.bounds {
            Some(previous) => Bounds {
                min: previous.min.min(bounds.min),
                max: previous.max.max(bounds.max),
            },
            None => bounds,
        });
    }

    /// Draw into a window frame or offscreen target using wgame's native batches.
    pub fn render(&self, target: &mut impl Target, navigation: &Camera) {
        target.clear(self.background);
        let mut camera = target.camera();
        if let Some(transform) = navigation.transform(self.bounds, target.size()) {
            camera = camera.transform(transform);
        }
        target.render_iter(&camera, self.scene.iter());
    }
}
