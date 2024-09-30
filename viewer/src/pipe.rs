use std::error::Error;

pub trait DrawPipe {
    type Error: Error + Send + 'static;
    /// Blocking wait for next draw command.
    fn receive(&mut self) -> Result<DrawCmd, Self::Error>;
}

pub struct DrawCmd {
    pub name: String,
    pub content: DrawContent,
}

pub enum DrawContent {
    Text(String),
    Plot {
        points: Vec<(f32, f32)>,
    },
    Bitmap {
        shape: (usize, usize),
        data: Vec<[u8; 3]>,
    },
}
