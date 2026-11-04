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

#[test]
fn preparing_and_discarding_memory_does_not_change_the_mind() {
    let mut genome = RnnGenome::default();
    genome.weights[28] = 0.5;
    genome.weights[32] = 1.0;
    let mut mind = Rnn::new(genome);
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    let (output, discarded) = mind.prepare(&mut cx, [0.0; 3]).unwrap();
    assert_eq!(output, [0.5_f32.tanh(), 0.0, 0.0]);
    assert_ne!(discarded, mind.memory);
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);

    let (repeated_output, update) = mind.prepare(&mut cx, [0.0; 3]).unwrap();
    assert_eq!((repeated_output, update), (output, discarded));
    <Rnn as Mind<Cpu<Xoshiro256PlusPlus>>>::apply(&mut mind, update);
    assert_eq!(mind.memory, update);
    assert_eq!(mind.genome, original.genome);
    assert_eq!(cx, original_cx);
}

#[test]
fn invalid_prepared_memory_fails_before_changing_the_mind() {
    let mut mind = Rnn::new(RnnGenome::default());
    let original = mind.clone();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
    };
    let original_cx = cx.clone();

    assert!(
        mind.prepare(&mut cx, [f32::NAN, 0.0, 0.0])
            .unwrap_err()
            .to_string()
            .contains("prepared recurrent memory")
    );
    assert_eq!(mind, original);
    assert!(mind.process(&mut cx, [f32::NAN, 0.0, 0.0]).is_err());
    assert_eq!(mind, original);
    assert_eq!(cx, original_cx);
}
