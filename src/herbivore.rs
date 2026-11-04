use std::{array, collections::BTreeMap};

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use crate::{
    Agent, AgentId, Context, Cpu, Genome, Mutate, Population,
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
    fn random<C: Context + ?Sized>(cx: &mut C) -> Self {
        Self {
            weights: array::from_fn(|_| cx.rng().random_range(-0.5..0.5)),
        }
    }
}

impl<C: Context + ?Sized> Genome<C> for HerbivoreGenome {}

/// Independent uniform noise in [-max_delta, max_delta) for each weight and bias.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightMutation {
    pub max_delta: f32,
}

impl Default for WeightMutation {
    fn default() -> Self {
        Self { max_delta: 0.02 }
    }
}

impl<C: Context + ?Sized> Mutate<C, WeightMutation> for HerbivoreGenome {
    fn mutate(&mut self, param: &WeightMutation, cx: &mut C) -> Result<()> {
        let delta = param.max_delta;
        ensure!(
            delta >= 0.0 && (2.0 * delta).is_finite(),
            "mutation max_delta must be nonnegative with a finite sampling range"
        );
        if delta > 0.0 {
            for weight in &mut self.weights {
                *weight += cx.rng().random_range(-delta..delta);
            }
        }
        Ok(())
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

impl<C: Context + ?Sized> Agent<C> for Herbivore {
    type Input = FoodObservation;
    type Output = MoveAction;

    fn process(&mut self, _cx: &mut C, obs: FoodObservation) -> Result<MoveAction> {
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

        Ok(MoveAction {
            direction: DVec2::new(output[0].into(), output[1].into()),
            speed_fraction: f64::from(output[2]).tanh().abs(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Herbivores {
    live: BTreeMap<AgentId, Herbivore>,
    cx: Cpu<Xoshiro256PlusPlus>,
    founder_genome: Option<HerbivoreGenome>,
}

impl Herbivores {
    pub fn new(genetic_seed: u64) -> Self {
        Self {
            live: BTreeMap::new(),
            cx: Cpu {
                rng: Xoshiro256PlusPlus::seed_from_u64(genetic_seed),
            },
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
                .unwrap_or_else(|| HerbivoreGenome::random(&mut self.cx)),
            Origin::Immigrant => HerbivoreGenome::random(&mut self.cx),
            Origin::Child { parent } => {
                let mut genome = self
                    .live
                    .get(&parent)
                    .expect("child requires a living parent")
                    .genome
                    .clone();
                genome
                    .mutate(&WeightMutation::default(), &mut self.cx)
                    .expect("the baseline mutation range is valid");
                genome
            }
        };
        self.live.insert(id, Herbivore::new(genome));
    }

    fn act(&mut self, id: AgentId, obs: &FoodObservation) -> MoveAction {
        self.live
            .get_mut(&id)
            .expect("only a living agent can act")
            .process(&mut self.cx, *obs)
            .expect("the scalar herbivore agent is infallible")
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
        let mut cx = Cpu {
            rng: Xoshiro256PlusPlus::seed_from_u64(1),
        };

        let first = herbivore
            .process(
                &mut cx,
                FoodObservation {
                    strength: 0.0,
                    gradient: DVec2::X,
                },
            )
            .unwrap();
        assert_eq!(first.direction, DVec2::X * f64::from(1.0_f32.tanh()));
        assert_eq!(first.speed_fraction, 1.0_f64.tanh());
        let second = herbivore
            .process(
                &mut cx,
                FoodObservation {
                    strength: 0.0,
                    gradient: DVec2::ZERO,
                },
            )
            .unwrap();
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
