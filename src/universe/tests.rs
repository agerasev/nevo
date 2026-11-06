use super::*;
use crate::{ecosystem::Event, mind::rnn::RnnGenome};
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

#[test]
fn checkpoint_continuation_preserves_memory_randomness_and_population_turnover() {
    let config = Config {
        initial_animals: 2,
        plants: 0,
        minimum_population: 2,
        ..Config::default()
    };
    let world: Ecosystem =
        Ecosystem::with_genetics(config.clone(), 23, Some(RnnGenome::default())).unwrap();
    let mut value = Value::serialized(&Universe::from_world(world).unwrap()).unwrap();
    let animals = field(&mut value, &["world", "animals"])
        .as_map_mut()
        .unwrap();
    *field(&mut animals[0].1, &["body", "energy"]) = Value::Float(-0.01);
    *field(&mut animals[1].1, &["body", "energy"]) = Value::Float(1.2);
    *field(&mut animals[1].1, &["mind", "brain", "memory"]) =
        Value::Array(vec![Value::Float(0.25); 4]);
    let state: Universe = value.deserialized().unwrap();
    let mut bytes = Vec::new();
    ciborium::into_writer(&state, &mut bytes).unwrap();
    let decoded: Universe = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(decoded, state);
    let mut original = state.restore(config.clone()).unwrap();
    let mut resumed = decoded.restore(config).unwrap();
    for tick in 1..=3 {
        let report = original.tick().unwrap();
        if tick == 1 {
            assert!(
                report
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::Death { .. }))
            );
            assert!(
                report
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::Immigrant { .. }))
            );
            assert!(
                report
                    .events
                    .iter()
                    .any(|e| matches!(e, Event::Birth { .. }))
            );
        }
        assert_eq!(resumed.tick().unwrap(), report);
        assert_eq!(*resumed.universe().read(), *original.universe().read());
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
