use super::*;
use crate::{
    agent::{AgentId, Population},
    environment::ecosystem::Origin,
};

#[test]
fn offspring_mutate_the_genome_and_start_with_fresh_memory() {
    let mut founder_genome = RnnGenome::default();
    founder_genome.weights[28] = 0.5;
    let mut population = Herbivores::with_founder_genome(17, founder_genome.clone());
    let (parent, child, immigrant) = (AgentId(1), AgentId(2), AgentId(3));
    population.spawn(parent, Origin::Founder);
    population.act(
        parent,
        &FoodObservation {
            strength: 0.0,
            gradient: DVec2::ZERO,
        },
    );
    let original = population.get(parent).unwrap().clone();
    assert_ne!(original.mind.memory, [0.0; 4]);

    population.spawn(child, Origin::Child { parent });
    let baby = population.get(child).unwrap();
    assert_eq!(population.get(parent), Some(&original));
    assert_eq!(baby.mind.memory, [0.0; 4]);
    assert_ne!(baby.mind.genome, founder_genome);
    for (inherited, original) in baby.mind.genome.weights.iter().zip(founder_genome.weights) {
        assert!((inherited - original).abs() <= 0.02 + f32::EPSILON);
    }

    population.spawn(immigrant, Origin::Immigrant);
    let migrant = population.get(immigrant).unwrap();
    assert_ne!(migrant.mind.genome, founder_genome);
    assert!(
        migrant
            .mind
            .genome
            .weights
            .iter()
            .all(|w| (-0.5..0.5).contains(w))
    );
    population.despawn(parent);
    assert!(population.get(parent).is_none());
    assert!(population.get(child).is_some());
}
