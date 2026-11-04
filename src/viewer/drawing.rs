use glam::{Affine3A, Vec2, Vec3};
use wgame::{
    Library,
    gfx::{Scene, Target},
    prelude::*,
    rgb::Rgb,
};

/// World-space region to fit into a viewport, preserving its aspect ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl Bounds {
    /// Invalid or degenerate bounds leave the target's default camera unchanged.
    pub(crate) fn transform(self, size: (u32, u32)) -> Option<Affine3A> {
        let half = (self.max - self.min) * 0.5;
        let center = self.min * 0.5 + self.max * 0.5;
        if size.0 == 0
            || size.1 == 0
            || !half.is_finite()
            || !center.is_finite()
            || half.min_element() <= 0.0
        {
            return None;
        }
        let aspect = size.0 as f32 / size.1 as f32;
        let scale = (aspect / half.x).min(1.0 / half.y) * 0.96;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        Some(
            Affine3A::from_scale(Vec3::new(scale, scale, 1.0))
                * Affine3A::from_translation((-center).extend(0.0)),
        )
    }
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
    pub fn render(&self, target: &mut impl Target) {
        target.clear(self.background);
        let mut camera = target.camera();
        if let Some(transform) = self
            .bounds
            .and_then(|bounds| bounds.transform(target.size()))
        {
            camera = camera.transform(transform);
        }
        target.render_iter(&camera, self.scene.iter());
    }
}
