use rand::SeedableRng;
use rand_xoshiro::Xoshiro256PlusPlus;

use super::*;
use crate::{actor::HandleAllocator, context::Cpu};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Counter {
    value: i32,
    pending: Option<i32>,
}

impl<C: Context + ?Sized> Actor<C> for Counter {
    type Observation = i32;
    type Action = i32;

    fn observe(&mut self, _cx: &mut C, observation: i32) -> Result<()> {
        ensure!(self.pending.is_none(), "unconsumed counter input");
        self.pending = Some(observation);
        Ok(())
    }

    fn act(&mut self, _cx: &mut C) -> Result<i32> {
        self.value += self
            .pending
            .take()
            .ok_or_else(|| anyhow::anyhow!("missing input"))?;
        Ok(self.value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct CounterFactory;

impl<C: Context + ?Sized> Factory<C, Counter> for CounterFactory {
    type Origin = i32;

    fn create(
        &self,
        _cx: &mut C,
        value: i32,
        _existing: &BTreeMap<Handle, Counter>,
    ) -> Result<Counter> {
        Ok(Counter {
            value,
            pending: None,
        })
    }
}

type Counters = Population<Counter, CounterFactory>;
type TestContext = Cpu<Xoshiro256PlusPlus>;

fn fixture() -> (Counters, TestContext, [Handle; 3]) {
    let mut allocator = HandleAllocator::new();
    let handles = std::array::from_fn(|_| allocator.allocate().unwrap());
    let mut population = Counters::default();
    let mut cx = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    population
        .observe(
            &mut cx,
            PopulationObservation::Lifecycle {
                members: handles[..2].to_vec(),
                changes: handles[..2]
                    .iter()
                    .map(|handle| Lifecycle::Spawn {
                        handle: *handle,
                        origin: 10,
                    })
                    .collect(),
            },
        )
        .unwrap();
    (population, cx, handles)
}

#[test]
fn population_preserves_input_order_and_serializes_pending_actions() {
    let (mut population, mut cx, [first, second, _]) = fixture();
    assert!(population.act(&mut cx).is_err());
    population
        .observe(
            &mut cx,
            PopulationObservation::Inputs {
                members: vec![second, first],
                inputs: vec![(second, 2), (first, 1)],
            },
        )
        .unwrap();
    let mut bytes = Vec::new();
    ciborium::into_writer(&population, &mut bytes).unwrap();
    let mut restored: Counters = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(
        restored.act(&mut cx).unwrap(),
        vec![(second, 12), (first, 11)]
    );
    assert_eq!(
        population.act(&mut cx).unwrap(),
        vec![(second, 12), (first, 11)]
    );
    assert_eq!(restored, population);
    assert!(population.act(&mut cx).is_err());
    assert_eq!(
        population
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn inputs_allow_inactive_members_but_reject_duplicate_or_unknown_handles() {
    let (mut population, mut cx, [first, second, unknown]) = fixture();
    for (members, inputs) in [
        (vec![first], vec![(first, 1)]),
        (vec![first, first, second], vec![(first, 1)]),
        (vec![first, second], vec![(unknown, 1)]),
        (vec![first, second], vec![(first, 1), (first, 2)]),
    ] {
        let before = population.clone();
        assert!(
            population
                .observe(&mut cx, PopulationObservation::Inputs { members, inputs })
                .is_err()
        );
        assert_eq!(population, before);
    }
    population
        .observe(
            &mut cx,
            PopulationObservation::Inputs {
                members: vec![first, second],
                inputs: vec![(second, 2)],
            },
        )
        .unwrap();
    assert_eq!(population.act(&mut cx).unwrap(), vec![(second, 12)]);
    assert_eq!(population.get(first).unwrap().value, 10);
}

#[test]
fn lifecycle_is_sequential_and_checked_without_scheduling_actions() {
    let (mut population, mut cx, [first, second, third]) = fixture();
    assert!(
        population
            .observe(
                &mut cx,
                PopulationObservation::Lifecycle {
                    members: vec![first, second],
                    changes: vec![Lifecycle::Spawn {
                        handle: first,
                        origin: 20
                    }],
                }
            )
            .is_err()
    );
    assert!(
        population
            .observe(
                &mut cx,
                PopulationObservation::Lifecycle {
                    members: vec![first, second],
                    changes: vec![Lifecycle::Despawn { handle: third }],
                }
            )
            .is_err()
    );
    population
        .observe(
            &mut cx,
            PopulationObservation::Lifecycle {
                members: vec![second, third],
                changes: vec![
                    Lifecycle::Despawn { handle: first },
                    Lifecycle::Spawn {
                        handle: third,
                        origin: 30,
                    },
                ],
            },
        )
        .unwrap();
    assert_eq!(
        population.members().collect::<Vec<_>>(),
        vec![second, third]
    );
    assert!(population.act(&mut cx).is_err());
    assert!(
        population
            .observe(
                &mut cx,
                PopulationObservation::Lifecycle {
                    members: vec![second],
                    changes: vec![],
                }
            )
            .is_err()
    );
}

#[test]
fn input_batch_must_be_consumed_before_another_observation() {
    let (mut population, mut cx, [first, second, _]) = fixture();
    population
        .observe(
            &mut cx,
            PopulationObservation::Inputs {
                members: vec![first, second],
                inputs: vec![],
            },
        )
        .unwrap();
    assert!(
        population
            .observe(
                &mut cx,
                PopulationObservation::Lifecycle {
                    members: vec![first, second],
                    changes: vec![],
                }
            )
            .is_err()
    );
    assert!(population.act(&mut cx).unwrap().is_empty());
    population
        .observe(
            &mut cx,
            PopulationObservation::Lifecycle {
                members: vec![first, second],
                changes: vec![],
            },
        )
        .unwrap();
}
