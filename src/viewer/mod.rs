//! Model-independent viewing through wgame. Simulation types opt into drawing.

mod controls;
mod drawing;
mod metrics;
mod window;

pub use drawing::{Bounds, Drawing, Label};
pub use window::{Options, Tab, show};

/// Collect an object's default presentation without borrowing its state afterward.
/// Drawing must not mutate simulation state, consume randomness, or present frames.
/// Compose children by calling their `draw` methods; keep collection inexpensive
/// because the viewer holds a read lock while collecting a complete scene.
pub trait Draw {
    fn draw(&self, drawing: &mut Drawing<'_>);
}

#[cfg(test)]
mod tests;
