//! Complete foraging state at a tick boundary. Serialization is an adapter;
//! the generic agent, environment, and runner contracts stay format-independent.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{foraging::ForagingWorld, herbivore::Herbivores, run::Run};

const FORMAT_VERSION: u32 = 1;
const WORLD_MODEL: &str = "foraging-sense-first-v1";
const AGENT_MODEL: &str = "herbivore-rnn-3-4-3-v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    format_version: u32,
    world_model: String,
    agent_model: String,
    world: ForagingWorld,
    population: Herbivores,
}

impl Snapshot {
    /// Call from the simulation owner between completed ticks.
    pub fn capture(run: &Run) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            world_model: WORLD_MODEL.into(),
            agent_model: AGENT_MODEL.into(),
            world: run.world().read().clone(),
            population: run.population.clone(),
        }
    }

    pub fn tick(&self) -> u64 {
        self.world.number()
    }

    /// Recreate synchronization only; never initialize agents or draw new seeds.
    pub fn restore(self) -> Result<Run> {
        ensure!(
            self.format_version == FORMAT_VERSION,
            "unsupported snapshot format version {}",
            self.format_version
        );
        ensure!(
            self.world_model == WORLD_MODEL,
            "unsupported world model {}",
            self.world_model
        );
        ensure!(
            self.agent_model == AGENT_MODEL,
            "unsupported agent model {}",
            self.agent_model
        );
        self.world.validate_checkpoint()?;
        self.population.validate_checkpoint()?;
        ensure!(
            self.world
                .animals()
                .keys()
                .copied()
                .eq(self.population.ids()),
            "world body IDs differ from population IDs"
        );
        Ok(Run::from_parts(self.world, self.population))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Population,
        foraging::{Event, Scenario},
        herbivore::HerbivoreGenome,
    };

    use super::*;

    fn round_trip(run: &Run) -> Run {
        let snapshot = Snapshot::capture(run);
        let mut bytes = Vec::new();
        ciborium::into_writer(&snapshot, &mut bytes).unwrap();
        let decoded: Snapshot = ciborium::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(decoded, snapshot);
        let mut reencoded = Vec::new();
        ciborium::into_writer(&decoded, &mut reencoded).unwrap();
        assert_eq!(reencoded, bytes);
        let restored = decoded.restore().unwrap();
        assert_eq!(&restored, run);
        restored
    }

    #[test]
    fn cbor_continuation_matches_every_report_and_complete_state() {
        for scenario in [Scenario::Debug, Scenario::Baseline] {
            for seed in [1, 42] {
                let mut uninterrupted = Run::new(scenario, seed);
                for _ in 0..73 {
                    uninterrupted.tick();
                }
                let mut resumed = round_trip(&uninterrupted);
                let mut turnover = false;
                for _ in 73..256 {
                    let report = uninterrupted.tick();
                    turnover |= report.events.iter().any(|event| {
                        matches!(event, Event::Death { .. } | Event::Immigrant { .. })
                    });
                    assert_eq!(resumed.tick(), report);
                    assert_eq!(resumed, uninterrupted);
                }
                assert!(turnover, "comparison must cross population turnover");
            }
        }
    }

    #[test]
    fn founder_template_memory_and_pending_deaths_survive() {
        let mut genome = HerbivoreGenome::default();
        genome.weights[0] = -0.0;
        genome.weights[1] = f32::from_bits(1);
        genome.weights[28] = 0.25;
        let mut population = Herbivores::with_founder_genome(17, genome);
        let world = ForagingWorld::new(Scenario::Debug, 23, &mut population);
        let mut run = Run::from_parts(world, population);
        let initial = round_trip(&run);
        assert_eq!(initial, run);
        run.tick();
        assert!(
            run.world()
                .read()
                .animals()
                .keys()
                .any(|&id| { run.population.get(id).unwrap().memory != [0.0; 4] })
        );
        let mut restored = round_trip(&run);
        for _ in 1..108 {
            assert_eq!(run.tick(), restored.tick());
        }
        assert!(
            run.world()
                .read()
                .animals()
                .values()
                .any(|body| body.energy < 0.0)
        );
        let mut restored = round_trip(&run);
        let report = run.tick();
        assert!(
            report
                .events
                .iter()
                .any(|event| matches!(event, Event::Death { .. }))
        );
        assert_eq!(restored.tick(), report);
        assert_eq!(restored, run);
    }

    #[test]
    fn incompatible_models_and_missing_agents_are_rejected() {
        let mut run = Run::new(Scenario::Debug, 1);
        let snapshot = Snapshot::capture(&run);
        let mut invalid = snapshot.clone();
        invalid.format_version += 1;
        assert!(
            invalid
                .restore()
                .unwrap_err()
                .to_string()
                .contains("version")
        );
        let mut invalid = snapshot.clone();
        invalid.world_model = "other-world".into();
        assert!(
            invalid
                .restore()
                .unwrap_err()
                .to_string()
                .contains("world model")
        );
        let mut invalid = snapshot;
        invalid.agent_model = "other-agent".into();
        assert!(
            invalid
                .restore()
                .unwrap_err()
                .to_string()
                .contains("agent model")
        );
        let id = *run.world().read().animals().keys().next().unwrap();
        run.population.despawn(id);
        assert!(
            Snapshot::capture(&run)
                .restore()
                .unwrap_err()
                .to_string()
                .contains("IDs")
        );
    }

    #[test]
    fn zero_rngs_and_wrong_weight_dimensions_are_rejected() {
        use ciborium::Value;

        fn field<'a>(value: &'a mut Value, path: &[&str]) -> &'a mut Value {
            let Some((head, tail)) = path.split_first() else {
                return value;
            };
            let child = &mut value
                .as_map_mut()
                .unwrap()
                .iter_mut()
                .find(|(key, _)| key.as_text() == Some(head))
                .unwrap()
                .1;
            field(child, tail)
        }

        let snapshot = Snapshot::capture(&Run::new(Scenario::Debug, 1));
        for path in [
            ["world", "rng", "s"].as_slice(),
            ["population", "cx", "rng", "s"].as_slice(),
        ] {
            let mut value = Value::serialized(&snapshot).unwrap();
            *field(&mut value, path) = Value::Array(vec![Value::from(0); 4]);
            let invalid: Snapshot = value.deserialized().unwrap();
            assert!(invalid.restore().unwrap_err().to_string().contains("RNG"));
        }
        let mut value = Value::serialized(&snapshot).unwrap();
        let agent = &mut field(&mut value, &["population", "live"])
            .as_map_mut()
            .unwrap()[0]
            .1;
        field(agent, &["genome", "weights"])
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(
            value
                .deserialized::<Snapshot>()
                .unwrap_err()
                .to_string()
                .contains("expected 47 weights")
        );
    }
}
