use std::future::Future;

use anyhow::Error;
use serde::{Deserialize, Serialize};

pub trait Pipe {
    fn send(&mut self, message: ControlMessage) -> impl Future<Output = Result<(), Error>> + Send;
    fn receive(&mut self) -> impl Future<Output = Result<ViewMessage, Error>> + Send;
}

#[derive(Serialize, Deserialize)]
pub struct ControlMessage;

#[derive(Serialize, Deserialize)]
pub struct ViewMessage {
    pub name: String,
    pub content: ViewContent,
}

#[derive(Serialize, Deserialize)]
pub enum ViewContent {
    Text(String),
    Plot {
        points: Vec<(f32, f32)>,
    },
    Bitmap {
        shape: (usize, usize),
        data: Vec<[u8; 3]>,
    },
}
