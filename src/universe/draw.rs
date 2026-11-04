use super::Universe;
use crate::viewer::{Draw, Drawing, Label};

impl<S: Draw> Draw for Universe<S> {
    fn draw(&self, drawing: &mut Drawing<'_>) {
        drawing
            .labels
            .push(Label::new(format!("Tick {}", self.step())));
        self.world().draw(drawing);
    }
}
