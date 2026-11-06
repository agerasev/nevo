use super::*;
use crate::{
    ecosystem::{Event, Obstacle},
    mind::rnn::RnnGenome,
};
use ciborium::Value;
use glam::Vec2;

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
    let mut config = Config {
        initial_animals: 2,
        plants: 2,
        minimum_population: 2,
        ..Config::default()
    };
    config.plant.patches = crate::ecosystem::PatchConfig {
        count: 3,
        growth_floor: 0.25,
        growth_peak: 1.75,
        ..Default::default()
    };
    config.plant.replenishment = Some(crate::ecosystem::ReplenishmentConfig {
        density_floor: 0.0001,
        density_peak: 0.001,
        ..Default::default()
    });
    config.obstacles = vec![Obstacle {
        position: Vec2::new(150.0, 0.0),
        radius: 20.0,
    }];
    let world: Ecosystem =
        Ecosystem::with_genetics(config.clone(), 23, Some(RnnGenome::default())).unwrap();
    let mut value = Value::serialized(&Universe::from_world(world).unwrap()).unwrap();
    let animals = field(&mut value, &["world", "animals"])
        .as_map_mut()
        .unwrap();
    *field(&mut animals[0].1, &["body", "energy"]) = Value::Float(-0.01);
    *field(&mut animals[1].1, &["body", "energy"]) = Value::Float(1.2);
    *field(&mut animals[1].1, &["body", "heading"]) = Value::Float(0.7);
    *field(&mut animals[1].1, &["body", "velocity"]) =
        Value::Array(vec![Value::Float(2.0), Value::Float(-1.0)]);
    *field(&mut animals[1].1, &["body", "angular_velocity"]) = Value::Float(0.3);
    *field(&mut animals[1].1, &["body", "position"]) =
        Value::Array(vec![Value::Float(-100.0), Value::Float(0.0)]);
    *field(&mut animals[1].1, &["mind", "brain", "memory"]) =
        Value::Array(vec![Value::Float(0.25); 4]);
    let plants = field(&mut value, &["world", "plants"])
        .as_array_mut()
        .unwrap();
    // A touching meal and a separate moving plant against the static obstacle.
    let meal_x = -100.0 + 10.0 * 1.2_f32.sqrt() + 10.0 - 0.001;
    *field(&mut plants[0], &["position"]) =
        Value::Array(vec![Value::Float(f64::from(meal_x)), Value::Float(0.0)]);
    *field(&mut plants[1], &["position"]) =
        Value::Array(vec![Value::Float(120.0), Value::Float(0.0)]);
    *field(&mut plants[1], &["velocity"]) =
        Value::Array(vec![Value::Float(4.0), Value::Float(2.0)]);
    *field(&mut plants[1], &["angular_velocity"]) = Value::Float(0.6);
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
                    .any(|e| matches!(e, Event::Meal { .. }))
            );
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
    let universe = Universe::new(crate::test_support::ecosystem("world/debug"), 1).unwrap();
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

#[test]
fn invalid_saved_kinematics_are_rejected() {
    let universe = Universe::new(crate::test_support::ecosystem("world/debug"), 1).unwrap();
    for (name, invalid) in [
        ("heading", Value::Float(f64::NAN)),
        ("heading", Value::Float(std::f64::consts::PI)),
        (
            "velocity",
            Value::Array(vec![Value::Float(f64::INFINITY), Value::Float(0.0)]),
        ),
        ("angular_velocity", Value::Float(f64::NEG_INFINITY)),
    ] {
        let mut value = Value::serialized(&universe).unwrap();
        let animal = &mut field(&mut value, &["world", "animals"])
            .as_map_mut()
            .unwrap()[0]
            .1;
        *field(animal, &["body", name]) = invalid;
        let decoded: Universe = value.deserialized().unwrap();
        assert!(decoded.validate().is_err(), "accepted invalid {name}");
    }
    for (name, invalid) in [
        (
            "velocity",
            Value::Array(vec![Value::Float(f64::NAN), Value::Float(0.0)]),
        ),
        ("angular_velocity", Value::Float(f64::INFINITY)),
    ] {
        let mut value = Value::serialized(&universe).unwrap();
        let plant = &mut field(&mut value, &["world", "plants"])
            .as_array_mut()
            .unwrap()[0];
        *field(plant, &[name]) = invalid;
        let decoded: Universe = value.deserialized().unwrap();
        assert!(decoded.validate().is_err(), "accepted invalid plant {name}");
    }
}

#[test]
fn invalid_saved_obstacles_are_rejected() {
    let mut config: Config = crate::test_support::ecosystem("world/debug");
    config.obstacles = vec![Obstacle {
        position: Vec2::new(150.0, 0.0),
        radius: 20.0,
    }];
    let universe = Universe::new(config, 1).unwrap();
    for (name, invalid) in [
        ("radius", Value::Float(0.0)),
        (
            "position",
            Value::Array(vec![Value::Float(f64::NAN), Value::Float(0.0)]),
        ),
    ] {
        let mut value = Value::serialized(&universe).unwrap();
        let obstacle = &mut field(&mut value, &["world", "obstacles"])
            .as_array_mut()
            .unwrap()[0];
        *field(obstacle, &[name]) = invalid;
        let decoded: Universe = value.deserialized().unwrap();
        assert!(
            decoded.validate().is_err(),
            "accepted invalid obstacle {name}"
        );
    }
}
