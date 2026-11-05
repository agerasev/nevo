use ciborium::Value;

use crate::{
    ecosystem::{Event, Placement},
    mind::rnn::{RnnGenome, WeightMutation},
    run::Run,
};

use super::*;

fn round_trip(run: &Run, config: Config) -> Run {
    let state = run.universe().read().clone();
    let mut bytes = Vec::new();
    ciborium::into_writer(&state, &mut bytes).unwrap();
    let decoded: Universe = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(decoded, state);
    let mut reencoded = Vec::new();
    ciborium::into_writer(&decoded, &mut reencoded).unwrap();
    assert_eq!(reencoded, bytes);
    let restored = decoded.restore(config).unwrap();
    assert_eq!(&restored, run);
    restored
}

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

#[test]
fn cbor_continuation_matches_every_report_and_complete_state() {
    for config in [
        crate::test_support::ecosystem::<Config>("debug"),
        crate::test_support::ecosystem::<Config>("baseline"),
    ] {
        for seed in [1, 42] {
            let mut uninterrupted = Run::new(config.clone(), seed).unwrap();
            for _ in 0..73 {
                uninterrupted.tick().unwrap();
            }
            let mut resumed = round_trip(&uninterrupted, config.clone());
            let mut turnover = false;
            for _ in 73..256 {
                let report = uninterrupted.tick().unwrap();
                turnover |= report
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Death { .. } | Event::Immigrant { .. }));
                assert_eq!(resumed.tick().unwrap(), report);
                assert_eq!(resumed, uninterrupted);
            }
            assert!(turnover, "comparison must cross population turnover");
        }
    }
}

#[test]
fn immigrant_template_memory_and_pending_deaths_survive() {
    let mut genome = RnnGenome::default();
    genome.weights[0] = -0.0;
    genome.weights[1] = f32::from_bits(1);
    genome.weights[28] = 0.25;
    let mut config = Config {
        initial_animals: 1,
        plants: 1,
        minimum_population: 0,
        ..Config::default()
    };
    config.immigration.energy = 0.975;
    config.immigration.position = Placement::Fixed(glam::DVec2::ZERO);
    config.plant.initial_position = Placement::Fixed(glam::DVec2::ZERO);
    config.genetics.mutation = WeightMutation { max_delta: 0.07 };
    let world = Ecosystem::with_genetics(config.clone(), 23, Some(genome)).unwrap();
    let universe = Universe::from_world(world).unwrap();
    let mut value = Value::serialized(&universe).unwrap();
    assert_eq!(
        field(&mut value, &["world", "mutation", "max_delta"]),
        &Value::Float(f64::from(0.07_f32))
    );
    assert!(!matches!(
        field(&mut value, &["world", "immigrant_genome"]),
        Value::Null
    ));
    let mut run = universe.restore(config.clone()).unwrap();
    assert_eq!(round_trip(&run, config.clone()), run);
    run.tick().unwrap();
    assert!(
        run.universe()
            .read()
            .world()
            .animals()
            .values()
            .any(|animal| animal.mind.brain.memory != [0.0; 4])
    );
    let mut restored = round_trip(&run, config.clone());
    for _ in 1..108 {
        assert_eq!(run.tick().unwrap(), restored.tick().unwrap());
    }
    assert!(
        run.universe()
            .read()
            .world()
            .animals()
            .values()
            .any(|animal| animal.body.energy < 0.0)
    );
    let mut restored = round_trip(&run, config);
    let report = run.tick().unwrap();
    assert!(
        report
            .events
            .iter()
            .any(|event| matches!(event, Event::Death { .. }))
    );
    assert_eq!(restored.tick().unwrap(), report);
    assert_eq!(restored, run);
}

#[test]
fn animals_cannot_deserialize_without_a_body_or_mind() {
    let universe = Universe::new(crate::test_support::ecosystem("debug"), 1).unwrap();
    for missing in ["body", "mind"] {
        let mut value = Value::serialized(&universe).unwrap();
        let animal = &mut field(&mut value, &["world", "animals"])
            .as_map_mut()
            .unwrap()[0]
            .1;
        animal
            .as_map_mut()
            .unwrap()
            .retain(|(key, _)| key.as_text() != Some(missing));
        assert!(
            value.deserialized::<Universe>().is_err(),
            "accepted animal without {missing}"
        );
    }
}

#[test]
fn zero_rngs_and_wrong_weight_dimensions_are_rejected() {
    let universe = Universe::new(crate::test_support::ecosystem("debug"), 1).unwrap();
    for path in [
        ["world", "rng", "s"].as_slice(),
        ["world", "context", "rng", "s"].as_slice(),
    ] {
        let mut value = Value::serialized(&universe).unwrap();
        *field(&mut value, path) = Value::Array(vec![Value::from(0); 4]);
        let invalid: Universe = value.deserialized().unwrap();
        assert!(
            invalid
                .restore(Config::default())
                .unwrap_err()
                .to_string()
                .contains("RNG")
        );
    }
    let mut value = Value::serialized(&universe).unwrap();
    let animal = &mut field(&mut value, &["world", "animals"])
        .as_map_mut()
        .unwrap()[0]
        .1;
    field(animal, &["mind", "brain", "genome", "weights"])
        .as_array_mut()
        .unwrap()
        .pop();
    match value.deserialized::<Universe>() {
        Err(_) => {}
        Ok(invalid) => assert!(invalid.validate().is_err()),
    }
}
