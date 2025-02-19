use std::{
    io::{Read, Write},
    marker::PhantomData,
};

use anyhow::Error;
use base64_serde::base64_serde_type;
use glam::{UVec2, Vec2};
use serde::{Deserialize, Serialize};
use serde_json::{de::IoRead, Deserializer, StreamDeserializer};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ControlMessage {
    Show,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewMessage {
    pub name: String,
    pub content: ViewContent,
}

base64_serde_type!(Base64Standard, base64::engine::general_purpose::STANDARD);

#[derive(Clone, Debug, Serialize, Deserialize)]
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

pub struct MessageReader<R: Read, M: Deserialize<'static>> {
    read: StreamDeserializer<'static, IoRead<R>, M>,
}

impl<R: Read, M: Deserialize<'static>> MessageReader<R, M> {
    pub fn new(read: R) -> Self {
        Self {
            read: Deserializer::from_reader(read).into_iter(),
        }
    }

    pub fn read_message(&mut self) -> Result<Option<M>, Error> {
        Ok(self.read.next().transpose()?)
    }
}

pub struct MessageWriter<W: Write, M: Serialize> {
    write: W,
    _ghost: PhantomData<M>,
}

impl<W: Write, M: Serialize> MessageWriter<W, M> {
    pub fn new(write: W) -> Self {
        Self {
            write,
            _ghost: PhantomData,
        }
    }

    pub fn write_message(&mut self, message: &M) -> Result<(), Error> {
        Ok(serde_json::to_writer(&mut self.write, message)?)
    }
}
