use candle::Tensor;

mod animal;
//mod world;

#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub vision_channels: usize,
    pub vision_size: usize,
    pub status_dim: usize,
    pub out_dim: usize,
}

#[derive(Clone, Debug)]
pub struct AgentInput {
    /// shape: (1, vision_channels, vision_size, vision_size)
    pub vision: Tensor,
    /// shape: (1, status_dim)
    pub status: Tensor,
}

#[derive(Clone, Debug)]
pub struct AgentOutput {
    /// shape: (1, out_dim)
    pub action: Tensor,
}

fn main() {}
