use super::*;
use crate::context::Cpu;
use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

#[test]
fn recurrent_units_read_previous_memory() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = 1.0; // input 0 -> hidden 0
    genome.weights[16] = 1.0; // previous hidden 0 -> hidden 1
    genome.weights[32] = 1.0; // hidden 0 -> output 0
    genome.weights[37] = 1.0; // hidden 1 -> output 1
    genome.weights[46] = -1.0; // output 2 bias
    let mut mind = Rnn::new(genome);
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(1),
    };
    let before = cx.clone();

    assert_eq!(
        mind.process(&mut cx, [1.0, 0.0, 0.0]).unwrap(),
        [1.0_f32.tanh(), 0.0, -1.0]
    );
    assert_eq!(
        mind.process(&mut cx, [0.0; 3]).unwrap(),
        [0.0, 1.0_f32.tanh().tanh(), -1.0]
    );
    assert_eq!(mind.memory, [0.0, 1.0_f32.tanh().tanh(), 0.0, 0.0]);
    assert_eq!(cx, before, "processing must not consume genetic draws");
}
