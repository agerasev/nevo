use anyhow::Result;
use candle::{Module, Tensor};
use candle_nn::{
    conv2d, layer_norm, linear, lstm, rnn::LSTMState, Conv2d, Conv2dConfig, LSTMConfig, LayerNorm,
    LayerNormConfig, Linear, VarBuilder, LSTM, RNN,
};
use nevo::{Agent, Context, Evolving, Genome, Mutate};
use rand::Rng;
use std::collections::HashMap;

use crate::{AgentConfig, AgentInput, AgentOutput};

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
pub struct BrainConfig {
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

struct Brain {
    vision: Vision,
    vision_norm: LayerNorm,
    status_norm: LayerNorm,
    rnn: LSTM,
    out_norm: LayerNorm,
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

impl BrainConfig {
    fn build(
        self,
        AgentConfig {
            vision_channels,
            vision_size,
            status_dim,
            out_dim,
        }: AgentConfig,
        vb: VarBuilder,
    ) -> candle::Result<Brain> {
        let (vision_out_size, out_channels) = self
            .vision
            .layers
            .iter()
            .fold((vision_size, vision_channels), |(in_size, _), config| {
                ((in_size - 1) / 2, config.out_channels)
            });
        let vision_out_dim = vision_out_size * vision_out_size * out_channels;
        Ok(Brain {
            vision: self.vision.build(vision_channels, vb.pp("vision"))?,
            vision_norm: layer_norm(
                vision_out_dim,
                LayerNormConfig::default(),
                vb.pp("vision_norm"),
            )?,
            status_norm: layer_norm(status_dim, LayerNormConfig::default(), vb.pp("status_norm"))?,
            rnn: lstm(
                vision_out_dim + status_dim,
                self.mem_size,
                LSTMConfig::default(),
                vb.pp("rnn"),
            )?,
            out_norm: layer_norm(self.mem_size, LayerNormConfig::default(), vb.pp("out_norm"))?,
            out: linear(self.mem_size, out_dim, vb.pp("out"))?,
        })
    }
}

impl Brain {
    fn forward(
        &self,
        mem: &mut LSTMState,
        vision: &Tensor,
        status: &Tensor,
    ) -> candle::Result<Tensor> {
        let vision_out = vision
            .apply(&self.vision)?
            .reshape((1, ()))?
            .apply(&self.vision_norm)?;
        let status_out = status.apply(&self.status_norm)?;
        let rnn_in = Tensor::cat(&[vision_out, status_out], 1)?;
        *mem = self.rnn.step(&rnn_in, mem)?;
        let rnn_out = mem.h();
        rnn_out.apply(&self.out_norm)?.apply(&self.out)
    }
}

#[derive(Clone, Debug)]
pub struct AnimalConfig {
    pub world: AgentConfig,
    pub brain: BrainConfig,
    pub weights: HashMap<String, Tensor>,
}

pub struct Animal {
    genome: AnimalConfig,
    brain: Brain,
    brain_state: LSTMState,
}

impl Genome for AnimalConfig {}

impl Mutate<f64> for AnimalConfig {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: &f64, rng: &mut R) -> Result<()> {
        self.weights.mutate(rate, rng)
    }
}

impl Evolving for Animal {
    type Genome = AnimalConfig;
    fn genome(&self) -> Self::Genome {
        self.genome.clone()
    }
    fn instance<C: Context>(genome: &Self::Genome, cx: &mut C) -> Result<Self> {
        let vb = VarBuilder::from_tensors(genome.weights.clone(), cx.dtype(), &cx.device());
        let brain = genome.brain.clone().build(genome.world.clone(), vb)?;
        let brain_state = brain.rnn.zero_state(1)?;
        Ok(Animal {
            genome: genome.clone(),
            brain,
            brain_state,
        })
    }
}

impl Agent for Animal {
    type Input = AgentInput;
    type Output = AgentOutput;
    fn process<C: Context>(&mut self, _cx: &mut C, input: Self::Input) -> Result<Self::Output> {
        let out = self
            .brain
            .forward(&mut self.brain_state, &input.vision, &input.status)?;

        // Detach state
        self.brain_state =
            LSTMState::new(self.brain_state.h().detach(), self.brain_state.c().detach());

        Ok(AgentOutput { action: out })
    }
}
