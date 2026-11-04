//! Initial layout and external population support for the foraging experiment.

use anyhow::{Result, ensure};
use glam::DVec2;
use rand::{Rng, SeedableRng};
use rand_xoshiro::{SplitMix64, Xoshiro256PlusPlus};

use super::{Policy, Universe};
use crate::{
    actor::{Actor, HandleAllocator, herbivore::Herbivores},
    context::Cpu,
    environment::ecosystem::{Admission, Ecosystem, Immigration, ImmigrationRequest, Scenario},
    run::Run,
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForagingPolicy {
    pub minimum_population: usize,
}

impl ForagingPolicy {
    pub fn new(minimum_population: usize) -> Self {
        Self { minimum_population }
    }
}

impl Policy<ImmigrationRequest> for ForagingPolicy {
    type Intervention = Immigration;

    fn respond(&mut self, request: ImmigrationRequest) -> Result<Immigration> {
        Ok(Immigration {
            count: self.minimum_population.saturating_sub(request.population),
        })
    }
}

impl Universe {
    pub fn new(scenario: Scenario, seed: u64) -> Result<Self> {
        let mut seeds = SplitMix64::seed_from_u64(seed);
        let world_seed = seeds.next_u64();
        let mut context = Cpu {
            rng: Xoshiro256PlusPlus::seed_from_u64(seeds.next_u64()),
        };
        let (arrivals, plants, minimum, admission) = match scenario {
            Scenario::Debug => (
                1,
                1,
                0,
                Admission {
                    position: Some(DVec2::ZERO),
                    energy: 390.0,
                },
            ),
            Scenario::Baseline => (20, 500, 20, Admission::default()),
        };
        let mut environment = Ecosystem::empty(plants, world_seed);
        let mut handles = HandleAllocator::default();
        let initial = environment.admit(arrivals, admission, &mut handles)?;
        environment.seed_plants(admission.position)?;
        let mut actor = Herbivores::default();
        actor.observe(&mut context, initial)?;
        Self::from_parts(
            0,
            environment,
            actor,
            context,
            ForagingPolicy::new(minimum),
            handles,
        )
    }

    pub fn validate_checkpoint(&self) -> Result<()> {
        self.validate()?;
        ensure!(
            self.context.rng.state() != [0; 32],
            "genetic RNG state is all zero"
        );
        ensure!(
            self.environment.animals().len() >= self.policy.minimum_population,
            "population is below its floor"
        );
        Ok(())
    }

    pub fn restore(self) -> Result<Run> {
        self.validate_checkpoint()?;
        Ok(Run::from_universe(self))
    }
}
