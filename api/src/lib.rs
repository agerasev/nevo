use std::{
    io::{Read, Write},
    marker::PhantomData,
};

use anyhow::Error;
use base64_serde::base64_serde_type;
use glam::{UVec2, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum ControlMessage {
    Show,
}

#[derive(Serialize, Deserialize)]
pub struct ViewMessage {
    pub name: String,
    pub content: ViewContent,
}

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);

#[derive(Serialize, Deserialize)]
pub enum ViewContent {
    Text(String),
    Plot {
        points: Vec<Vec2>,
    },
    Bitmap {
        shape: UVec2,
        /// Flattened array of bytes of shape: `(shape.y, shape.x, 3)`
        #[serde(with = "Base64Standard")]
        data: Vec<u8>,
    },
}

pub struct MessageReader<R: Read, M: for<'de> Deserialize<'de>> {
    read: R,
    _ghost: PhantomData<M>,
}

impl<R: Read, M: for<'de> Deserialize<'de>> MessageReader<R, M> {
    pub fn new(read: R) -> Self {
        Self {
            read,
            _ghost: PhantomData,
        }
    }

    pub fn read_message(&mut self) -> Result<M, Error> {
        Ok(serde_json::from_reader(&mut self.read)?)
    }
}

pub struct MessageWriter<W: Write, M: Serialize> {
    write: W,
    _ghost: PhantomData<M>,
}

impl<R: Write, M: Serialize> MessageWriter<R, M> {
    pub fn new(write: R) -> Self {
        Self {
            write,
            _ghost: PhantomData,
        }
    }

    pub fn write_message(&mut self, message: &M) -> Result<(), Error> {
        Ok(serde_json::to_writer(&mut self.write, message)?)
    }
}
