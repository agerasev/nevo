use std::{array, collections::BTreeMap};

use glam::DVec2;
use rand::{RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::{
    AgentId, Controller, Population,
    foraging::{FoodObservation, MoveAction, Origin},
};

/// The July 26 controller's row-major input, recurrent and output parameters:
/// 4×3 input weights, 4×4 recurrent weights, 4 biases, 3×4 output weights, 3 biases.
#[derive(Clone, Debug, PartialEq)]
pub struct HerbivoreGenome {
    pub weights: [f32; 47],
}

impl Default for HerbivoreGenome {
    fn default() -> Self {
        Self { weights: [0.0; 47] }
    }
}

impl HerbivoreGenome {
    fn random(rng: &mut Xoshiro256PlusPlus) -> Self {
        Self {
            weights: array::from_fn(|_| rng.random_range(-0.5..0.5)),
        }
    }

    fn offspring(&self, rng: &mut Xoshiro256PlusPlus) -> Self {
        Self {
            weights: self
                .weights
                .map(|weight| weight + rng.random_range(-0.02..0.02)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Herbivore {
    pub genome: HerbivoreGenome,
    pub memory: [f32; 4],
}

impl Herbivore {
    pub fn new(genome: HerbivoreGenome) -> Self {
        Self {
            genome,
            memory: [0.0; 4],
        }
    }
}

fn matrix_vector<const IN: usize, const OUT: usize>(
    weights: &[f32],
    input: &[f32; IN],
) -> [f32; OUT] {
    debug_assert_eq!(weights.len(), IN * OUT);
    array::from_fn(|i| {
        weights[i * IN..(i + 1) * IN]
            .iter()
            .zip(input)
            .map(|(weight, value)| weight * value)
            .sum()
    })
}

impl Controller for Herbivore {
    type Observation = FoodObservation;
    type Action = MoveAction;

    fn act(&mut self, obs: &FoodObservation) -> MoveAction {
        let weights = &self.genome.weights;
        let input = [
            obs.gradient.x as f32,
            obs.gradient.y as f32,
            obs.strength as f32,
        ];
        let sensed: [f32; 4] = matrix_vector(&weights[..12], &input);
        // All four recurrent units read the previous tick's memory. The C++ dot
        // product aliased its input and output, overwriting components mid-update.
        let recalled: [f32; 4] = matrix_vector(&weights[12..28], &self.memory);
        self.memory = array::from_fn(|i| (recalled[i] + sensed[i] + weights[28 + i]).tanh());
        let mut output: [f32; 3] = matrix_vector(&weights[32..44], &self.memory);
        for (value, bias) in output.iter_mut().zip(&weights[44..]) {
            *value += bias;
        }

        MoveAction {
            direction: DVec2::new(output[0].into(), output[1].into()),
            speed_fraction: f64::from(output[2]).tanh().abs(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Herbivores {
    live: BTreeMap<AgentId, Herbivore>,
    rng: Xoshiro256PlusPlus,
    founder_genome: Option<HerbivoreGenome>,
}

impl Herbivores {
    pub fn new(genetic_seed: u64) -> Self {
        Self {
            live: BTreeMap::new(),
            rng: Xoshiro256PlusPlus::seed_from_u64(genetic_seed),
            founder_genome: None,
        }
    }

    /// Use the same known genome for every founder. Immigrants still draw random genomes.
    pub fn with_founder_genome(genetic_seed: u64, genome: HerbivoreGenome) -> Self {
        Self {
            founder_genome: Some(genome),
            ..Self::new(genetic_seed)
        }
    }

    pub fn get(&self, id: AgentId) -> Option<&Herbivore> {
        self.live.get(&id)
    }
}

impl Population<FoodObservation, MoveAction, Origin> for Herbivores {
    fn spawn(&mut self, id: AgentId, origin: Origin) {
        assert!(!self.live.contains_key(&id), "agent already alive: {id:?}");
        let genome = match origin {
            Origin::Founder => self
                .founder_genome
                .clone()
                .unwrap_or_else(|| HerbivoreGenome::random(&mut self.rng)),
            Origin::Immigrant => HerbivoreGenome::random(&mut self.rng),
            Origin::Child { parent } => self
                .live
                .get(&parent)
                .expect("child requires a living parent")
                .genome
                .offspring(&mut self.rng),
        };
        self.live.insert(id, Herbivore::new(genome));
    }

    fn act(&mut self, id: AgentId, obs: &FoodObservation) -> MoveAction {
        self.live
            .get_mut(&id)
            .expect("only a living agent can act")
            .act(obs)
    }

    fn despawn(&mut self, id: AgentId) {
        self.live
            .remove(&id)
            .expect("only a living agent can be removed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurrent_units_read_previous_memory() {
        let mut genome = HerbivoreGenome::default();
        genome.weights[0] = 1.0; // gradient.x -> hidden 0
        genome.weights[16] = 1.0; // previous hidden 0 -> hidden 1
        genome.weights[32] = 1.0; // hidden 0 -> direction.x
        genome.weights[37] = 1.0; // hidden 1 -> direction.y
        genome.weights[46] = -1.0; // speed bias
        let mut herbivore = Herbivore::new(genome);

        let first = herbivore.act(&FoodObservation {
            strength: 0.0,
            gradient: DVec2::X,
        });
        assert_eq!(first.direction, DVec2::X * f64::from(1.0_f32.tanh()));
        assert_eq!(first.speed_fraction, 1.0_f64.tanh());
        let second = herbivore.act(&FoodObservation {
            strength: 0.0,
            gradient: DVec2::ZERO,
        });
        assert_eq!(
            second.direction,
            DVec2::Y * f64::from(1.0_f32.tanh().tanh())
        );
        assert_eq!(herbivore.memory, [0.0, 1.0_f32.tanh().tanh(), 0.0, 0.0]);
    }

    #[test]
    fn offspring_mutate_the_genome_and_start_with_fresh_memory() {
        let mut founder_genome = HerbivoreGenome::default();
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
        assert_ne!(original.memory, [0.0; 4]);

        population.spawn(child, Origin::Child { parent });
        let baby = population.get(child).unwrap();
        assert_eq!(population.get(parent), Some(&original));
        assert_eq!(baby.memory, [0.0; 4]);
        assert_ne!(baby.genome, founder_genome);
        for (inherited, original) in baby.genome.weights.iter().zip(founder_genome.weights) {
            assert!((inherited - original).abs() <= 0.02 + f32::EPSILON);
        }

        population.spawn(immigrant, Origin::Immigrant);
        let migrant = population.get(immigrant).unwrap();
        assert_ne!(migrant.genome, founder_genome);
        assert!(
            migrant
                .genome
                .weights
                .iter()
                .all(|w| (-0.5..0.5).contains(w))
        );
        population.despawn(parent);
        assert!(population.get(parent).is_none());
        assert!(population.get(child).is_some());
    }
}
