use std::collections::HashMap;

use anyhow::Result;
use candle::{Module, Tensor};
use candle_nn::{
    Conv2d, Conv2dConfig, LSTM, LSTMConfig, Linear, RNN, VarBuilder, VarMap, conv2d, linear, lstm,
    rnn::LSTMState,
};

use nevo_core::{Agent, Candle as Cx, Context, Genome, Mutate, nn::init::DetermVarMap};

#[derive(Clone, Debug)]
pub struct InteractionConfig {
    pub vision_channels: usize,
    pub vision_size: usize,
    pub status_dim: usize,
    pub out_dim: usize,
}

#[derive(Clone, Debug)]
pub struct Perception {
    /// shape: (1, vision_channels, vision_size, vision_size)
    pub vision: Tensor,
    /// shape: (1, status_dim)
    pub status: Tensor,
}

#[derive(Clone, Debug)]
pub struct Decision {
    /// shape: (1, out_dim)
    pub action: Tensor,
}

#[derive(Clone, Debug)]
pub struct VisionLayerConfig {
    pub out_channels: usize,
    pub kernel_size: usize,
    pub max_pool: usize,
}

#[derive(Clone, Debug)]
pub struct VisionConfig {
    pub layers: Vec<VisionLayerConfig>,
}

#[derive(Clone, Debug)]
pub struct MindConfig {
    pub vision: VisionConfig,
    pub mem_size: usize,
}

struct VisionLayer {
    conv: Conv2d,
    max_pool: usize,
}

struct Vision {
    layers: Vec<VisionLayer>,
}

struct Mind {
    vision: Vision,
    rnn: LSTM,
    out: Linear,
}

impl VisionConfig {
    fn build(self, mut in_channels: usize, vb: VarBuilder) -> candle::Result<Vision> {
        Ok(Vision {
            layers: self
                .layers
                .into_iter()
                .enumerate()
                .map(
                    |(
                        i,
                        VisionLayerConfig {
                            out_channels,
                            kernel_size,
                            max_pool,
                        },
                    )| {
                        let conv = conv2d(
                            in_channels,
                            out_channels,
                            kernel_size,
                            Conv2dConfig::default(),
                            vb.pp(i),
                        )?;
                        in_channels = out_channels;
                        Ok(VisionLayer { conv, max_pool })
                    },
                )
                .collect::<candle::Result<_>>()?,
        })
    }
}

impl Module for Vision {
    fn forward(&self, xs: &Tensor) -> candle::Result<Tensor> {
        let mut xs = xs.clone();
        for layer in self.layers.iter() {
            xs = xs.apply(&layer.conv)?.max_pool2d(layer.max_pool)?;
        }
        Ok(xs)
    }
}

impl MindConfig {
    fn build(
        self,
        InteractionConfig {
            vision_channels,
            vision_size,
            status_dim,
            out_dim,
        }: InteractionConfig,
        vb: VarBuilder,
    ) -> candle::Result<Mind> {
        let (vision_out_size, out_channels) = self
            .vision
            .layers
            .iter()
            .fold((vision_size, vision_channels), |(in_size, _), config| {
                ((in_size - 1) / 2, config.out_channels)
            });
        let vision_out_dim = vision_out_size * vision_out_size * out_channels;
        Ok(Mind {
            vision: self.vision.build(vision_channels, vb.pp("vision"))?,
            rnn: lstm(
                vision_out_dim + status_dim,
                self.mem_size,
                LSTMConfig::default(),
                vb.pp("rnn"),
            )?,
            out: linear(self.mem_size, out_dim, vb.pp("out"))?,
        })
    }
}

impl Mind {
    fn forward(
        &self,
        mem: &mut LSTMState,
        vision: &Tensor,
        status: &Tensor,
    ) -> candle::Result<Tensor> {
        let vision = vision.apply(&self.vision)?.reshape((1, ()))?;
        let rnn_in = Tensor::cat(&[&vision, status], 1)?;
        *mem = self.rnn.step(&rnn_in, mem)?;
        let rnn_out = mem.h();
        rnn_out.apply(&self.out)
    }
}

#[derive(Clone, Debug)]
pub struct MindGenome {
    pub world: InteractionConfig,
    pub mind: MindConfig,
    pub weights: HashMap<String, Tensor>,
}

pub struct Brain {
    genome: MindGenome,
    mind: Mind,
    state: LSTMState,
}

impl Genome<Cx> for MindGenome {}

impl Mutate<Cx, f64> for MindGenome {
    fn mutate(&mut self, rate: &f64, cx: &mut Cx) -> Result<()> {
        self.weights.mutate(rate, cx)
    }
}

impl Brain {
    pub fn new(genome: &MindGenome, cx: &mut Cx) -> Result<Self> {
        let vb = VarBuilder::from_tensors(genome.weights.clone(), cx.dtype(), &cx.device());
        let brain = genome.mind.clone().build(genome.world.clone(), vb)?;
        let brain_state = brain.rnn.zero_state(1)?;
        Ok(Brain {
            genome: genome.clone(),
            mind: brain,
            state: brain_state,
        })
    }
}

impl Agent<Cx> for Brain {
    type Input = Perception;
    type Output = Decision;
    fn process(&mut self, _cx: &mut Cx, input: Self::Input) -> Result<Self::Output> {
        let out = self
            .mind
            .forward(&mut self.state, &input.vision, &input.status)?;

        // Detach state
        self.state = LSTMState::new(self.state.h().detach(), self.state.c().detach());

        Ok(Decision { action: out })
    }
}

impl MindGenome {
    pub fn new(cx: &mut Cx, world: InteractionConfig, mind: MindConfig) -> Result<Self> {
        let dtype = cx.dtype();
        let device = cx.device();
        let varmap = DetermVarMap::new(VarMap::new(), cx.rng());
        mind.clone().build(
            world.clone(),
            VarBuilder::from_backend(Box::new(varmap.clone()), dtype, device),
        )?;
        let weights = varmap
            .data()
            .try_lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_detached_tensor()))
            .collect();
        Ok(Self {
            world,
            mind,
            weights,
        })
    }
}
