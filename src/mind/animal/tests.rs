use super::*;

#[test]
fn constructing_from_a_genome_resets_memory() {
    let mut parent = AnimalMind::new(RnnGenome::default()).unwrap();
    parent.brain.memory.fill(0.5);
    let child = AnimalMind::try_from(parent.as_ref().clone()).unwrap();
    assert_eq!(child.as_ref(), parent.as_ref());
    assert_eq!(child.brain.memory, [0.0; 4]);
    assert_eq!(parent.brain.memory, [0.5; 4]);
}
