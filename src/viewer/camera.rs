use glam::{Affine3A, Vec2, Vec3};
use wgame::canvas::{Button, CanvasInput, Event, Key};

use super::Bounds;

#[derive(Clone, Copy, Debug)]
struct View {
    center: Vec2,
    units_per_point: f32,
}

impl View {
    fn fit(bounds: Bounds, size: Vec2) -> Option<Self> {
        let half = (bounds.max - bounds.min) * 0.5;
        let center = bounds.min * 0.5 + bounds.max * 0.5;
        let units_per_point = 2.0 * (half / size).max_element() / 0.96;
        (half.is_finite()
            && half.min_element() > 0.0
            && center.is_finite()
            && units_per_point.is_finite()
            && units_per_point > 0.0)
            .then_some(Self {
                center,
                units_per_point,
            })
    }

    fn world_at(self, point: Vec2, size: Vec2) -> Vec2 {
        self.center + (point - size * 0.5) * Vec2::new(1.0, -1.0) * self.units_per_point
    }
}

/// Viewer-owned navigation, independent of simulation state.
///
/// Starts in automatic fit mode. Dragging or scrolling fixes a manual center and
/// world units per logical pixel, preserving magnification across resizes and DPI
/// changes. Call `update` once per visible canvas frame, after UI layout.
#[derive(Debug, Default)]
pub struct Camera {
    view: Option<View>,
    viewport: Vec2,
    pointer: Option<Vec2>,
    dragging: bool,
}

impl Camera {
    /// Restore automatic fitting to the drawing's bounds.
    pub fn fit(&mut self) {
        self.view = None;
        self.cancel_gesture();
    }

    pub(super) fn cancel_gesture(&mut self) {
        self.pointer = None;
        self.dragging = false;
    }

    /// Consume canvas-local logical input. The host owns pointer capture and
    /// cancels gestures when focus or canvas geometry changes.
    pub fn update(&mut self, bounds: Option<Bounds>, size: (f64, f64), input: &CanvasInput) {
        let size = Vec2::new(size.0 as f32, size.1 as f32);
        if !valid_size(size) {
            self.cancel_gesture();
            return;
        }
        self.viewport = size;
        for event in &input.events {
            match *event {
                Event::Moved(point) => self.move_pointer(point, bounds),
                Event::Button {
                    button: Button::Primary,
                    pressed,
                    position,
                } => {
                    self.move_pointer(position, bounds);
                    self.dragging = pressed && position.is_finite();
                }
                Event::Scroll(delta) if delta.y.is_finite() && delta.y != 0.0 => {
                    if let Some(point) = self.pointer.or(input.pointer) {
                        self.zoom(point, delta.y, bounds);
                    }
                }
                Event::Key {
                    key: Key::Home,
                    pressed: true,
                    repeat: false,
                } => self.fit(),
                Event::Cancelled | Event::Focused(false) => self.cancel_gesture(),
                _ => {}
            }
        }
    }

    fn current_view(&self, bounds: Option<Bounds>, size: Vec2) -> Option<View> {
        self.view.or_else(|| View::fit(bounds?, size))
    }

    fn move_pointer(&mut self, point: Vec2, bounds: Option<Bounds>) {
        if !point.is_finite() {
            self.cancel_gesture();
            return;
        }
        if self.dragging
            && let Some(previous) = self.pointer
            && point != previous
            && let Some(mut view) = self.current_view(bounds, self.viewport)
        {
            view.center -= (point - previous) * Vec2::new(1.0, -1.0) * view.units_per_point;
            if view.center.is_finite() {
                self.view = Some(view);
            }
        }
        self.pointer = Some(point);
    }

    fn zoom(&mut self, point: Vec2, scroll: f32, bounds: Option<Bounds>) {
        if !point.is_finite() {
            return;
        }
        if let Some(mut view) = self.current_view(bounds, self.viewport) {
            let anchor = view.world_at(point, self.viewport);
            // Clamp before moving the center, so the cursor stays anchored even
            // at the zoom limits. Limits also prevent singular camera matrices.
            view.units_per_point =
                (view.units_per_point * (-scroll * 0.005).exp()).clamp(1e-4, 1e8);
            view.center += anchor - view.world_at(point, self.viewport);
            if view.center.is_finite() {
                self.view = Some(view);
            }
        }
    }

    pub(super) fn transform(&self, bounds: Option<Bounds>, size: (u32, u32)) -> Option<Affine3A> {
        let physical = Vec2::new(size.0 as f32, size.1 as f32);
        if !valid_size(physical) {
            return None;
        }
        let size = if self.view.is_some() {
            self.viewport
        } else {
            physical
        };
        let view = self.current_view(bounds, size)?;
        let scale = 2.0 / (size.y * view.units_per_point);
        let transform = Affine3A::from_scale(Vec3::new(scale, scale, 1.0))
            * Affine3A::from_translation((-view.center).extend(0.0));
        (scale.is_finite() && scale > 0.0 && transform.is_finite()).then_some(transform)
    }
}

fn valid_size(size: Vec2) -> bool {
    size.is_finite() && size.min_element() > 0.0
}
