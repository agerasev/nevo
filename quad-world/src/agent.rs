use anyhow::Result;
use candle::{Module, Tensor};
use candle_nn::{
    conv2d, layer_norm, linear, lstm, rnn::LSTMState, Conv2d, Conv2dConfig, LSTMConfig, LayerNorm,
    LayerNormConfig, Linear, VarBuilder, VarMap, LSTM, RNN,
};
use nevo::{nn::init::DetVarMap, Agent, Context, Evolving, Genome, Mutate};
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

impl MindConfig {
    fn build(
        self,
        AgentConfig {
            vision_channels,
            vision_size,
            status_dim,
            out_dim,
        }: AgentConfig,
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

impl Mind {
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
pub struct AnimalGenome {
    pub world: AgentConfig,
    pub mind: MindConfig,
    pub weights: HashMap<String, Tensor>,
}

pub struct AnimalBrain {
    genome: AnimalGenome,
    mind: Mind,
    state: LSTMState,
}

impl Genome for AnimalGenome {}

impl Mutate<f64> for AnimalGenome {
    fn mutate<R: Rng + ?Sized>(&mut self, rate: &f64, rng: &mut R) -> Result<()> {
        self.weights.mutate(rate, rng)
    }
}

impl Evolving for AnimalBrain {
    type Genome = AnimalGenome;
    fn genome(&self) -> Self::Genome {
        self.genome.clone()
    }
    fn instance<C: Context>(cx: &mut C, genome: &Self::Genome) -> Result<Self> {
        let vb = VarBuilder::from_tensors(genome.weights.clone(), cx.dtype(), &cx.device());
        let brain = genome.mind.clone().build(genome.world.clone(), vb)?;
        let brain_state = brain.rnn.zero_state(1)?;
        Ok(AnimalBrain {
            genome: genome.clone(),
            mind: brain,
            state: brain_state,
        })
    }
}

impl Agent for AnimalBrain {
    type Input = AgentInput;
    type Output = AgentOutput;
    fn process<C: Context>(&mut self, _cx: &mut C, input: Self::Input) -> Result<Self::Output> {
        let out = self
            .mind
            .forward(&mut self.state, &input.vision, &input.status)?;

        // Detach state
        self.state = LSTMState::new(self.state.h().detach(), self.state.c().detach());

        Ok(AgentOutput { action: out })
    }
}

impl AnimalGenome {
    pub fn new<C: Context>(cx: &mut C, world: AgentConfig, mind: MindConfig) -> Result<Self> {
        let dtype = cx.dtype();
        let device = cx.device();
        let varmap = DetVarMap::new(VarMap::new(), cx.rng());
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
