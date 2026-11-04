use ciborium::Value;
use rand::SeedableRng;

use crate::{
    actor::Actor,
    environment::ecosystem::{Admission, Event, Scenario},
    run::Run,
};

use super::*;

fn round_trip(run: &Run) -> Run {
    let state = run.universe().read().clone();
    let mut bytes = Vec::new();
    ciborium::into_writer(&state, &mut bytes).unwrap();
    let decoded: Universe = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(decoded, state);
    let mut reencoded = Vec::new();
    ciborium::into_writer(&decoded, &mut reencoded).unwrap();
    assert_eq!(reencoded, bytes);
    let restored = decoded.restore().unwrap();
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
    for scenario in [Scenario::Debug, Scenario::Baseline] {
        for seed in [1, 42] {
            let mut uninterrupted = Run::new(scenario, seed).unwrap();
            for _ in 0..73 {
                uninterrupted.tick().unwrap();
            }
            let mut resumed = round_trip(&uninterrupted);
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
    let mut actor = Herbivores::with_immigrant_genome(genome);
    let mut context = Cpu {
        rng: Xoshiro256PlusPlus::seed_from_u64(17),
    };
    let mut handles = HandleAllocator::default();
    let mut environment = Ecosystem::empty(1, 23);
    actor
        .observe(
            &mut context,
            environment
                .admit(
                    1,
                    Admission {
                        position: Some(glam::DVec2::ZERO),
                        energy: 390.0,
                    },
                    &mut handles,
                )
                .unwrap(),
        )
        .unwrap();
    environment.seed_plants(Some(glam::DVec2::ZERO)).unwrap();
    let universe = Universe::from_parts(
        0,
        environment,
        actor,
        context,
        ForagingPolicy::new(0),
        handles,
    )
    .unwrap();
    let mut run = Run::from_universe(universe);
    assert_eq!(round_trip(&run), run);
    run.tick().unwrap();
    assert!(
        run.universe()
            .read()
            .actor()
            .iter()
            .any(|(_, actor)| actor.mind.memory != [0.0; 4])
    );
    let mut restored = round_trip(&run);
    for _ in 1..108 {
        assert_eq!(run.tick().unwrap(), restored.tick().unwrap());
    }
    assert!(
        run.universe()
            .read()
            .environment()
            .animals()
            .values()
            .any(|body| body.energy < 0.0)
    );
    let mut restored = round_trip(&run);
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
fn missing_actors_are_rejected_at_the_universe_boundary() {
    let universe = Universe::new(Scenario::Debug, 1).unwrap();
    let mut value = Value::serialized(&universe).unwrap();
    field(&mut value, &["actor", "live"])
        .as_map_mut()
        .unwrap()
        .clear();
    let invalid: Universe = value.deserialized().unwrap();
    assert!(invalid.restore().is_err());
}

#[test]
fn zero_rngs_and_wrong_weight_dimensions_are_rejected() {
    let universe = Universe::new(Scenario::Debug, 1).unwrap();
    for path in [
        ["environment", "rng", "s"].as_slice(),
        ["context", "rng", "s"].as_slice(),
    ] {
        let mut value = Value::serialized(&universe).unwrap();
        *field(&mut value, path) = Value::Array(vec![Value::from(0); 4]);
        let invalid: Universe = value.deserialized().unwrap();
        assert!(invalid.restore().unwrap_err().to_string().contains("RNG"));
    }
    let mut value = Value::serialized(&universe).unwrap();
    let actor = &mut field(&mut value, &["actor", "live"]).as_map_mut().unwrap()[0].1;
    field(actor, &["mind", "genome", "weights"])
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        value
            .deserialized::<Universe>()
            .unwrap_err()
            .to_string()
            .contains("expected 47 weights")
    );
}

// Build the previous wire layout, independently of the importer, to exercise all
// migrated fields and model guards without committing a large generated fixture.
fn legacy_value(universe: &Universe) -> Value {
    let mut state = Value::serialized(universe).unwrap();
    let mut world = field(&mut state, &["environment"]).clone();
    world.as_map_mut().unwrap().extend([
        (Value::from("number"), Value::from(universe.step())),
        (
            Value::from("next_animal"),
            Value::from(universe.handles().next_serial()),
        ),
        (
            Value::from("immigrant_floor"),
            field(&mut state, &["policy", "minimum_population"]).clone(),
        ),
    ]);
    let live = field(&mut state, &["actor", "live"])
        .as_map()
        .unwrap()
        .iter()
        .map(|(key, actor)| {
            let mut actor = actor.clone();
            (key.clone(), field(&mut actor, &["mind"]).clone())
        })
        .collect();
    let population = Value::Map(vec![
        (Value::from("live"), Value::Map(live)),
        (Value::from("cx"), field(&mut state, &["context"]).clone()),
        (
            Value::from("founder_genome"),
            field(&mut state, &["actor", "factory", "immigrant_genome"]).clone(),
        ),
    ]);
    Value::Map(vec![
        (Value::from("format_version"), Value::from(1)),
        (Value::from("world_model"), Value::from(WORLD_MODEL)),
        (Value::from("agent_model"), Value::from(AGENT_MODEL)),
        (Value::from("world"), world),
        (Value::from("population"), population),
    ])
}

#[test]
fn original_snapshot_layout_imports_without_changing_continuation() {
    let mut reference = Run::new(Scenario::Baseline, 42).unwrap();
    for _ in 0..73 {
        reference.tick().unwrap();
    }
    let legacy: LegacySnapshot = legacy_value(&reference.universe().read())
        .deserialized()
        .unwrap();
    let mut imported = legacy.into_universe().unwrap().restore().unwrap();
    assert_eq!(imported, reference);
    for _ in 0..64 {
        assert_eq!(reference.tick().unwrap(), imported.tick().unwrap());
    }
    assert_eq!(imported, reference);
}

#[test]
fn original_snapshot_version_and_model_guards_remain_enforced() {
    let universe = Universe::new(Scenario::Debug, 1).unwrap();
    for (name, replacement) in [
        ("format_version", Value::from(2)),
        ("world_model", Value::from("other-world")),
        ("agent_model", Value::from("other-agent")),
    ] {
        let mut value = legacy_value(&universe);
        *field(&mut value, &[name]) = replacement;
        let invalid: LegacySnapshot = value.deserialized().unwrap();
        assert!(invalid.into_universe().is_err(), "accepted invalid {name}");
    }
}

#[test]
fn old_founder_template_does_not_become_an_immigration_template() {
    let universe = Universe::new(Scenario::Baseline, 1).unwrap();
    let mut value = legacy_value(&universe);
    *field(&mut value, &["population", "founder_genome"]) =
        Value::serialized(&RnnGenome::default()).unwrap();
    let imported: LegacySnapshot = value.deserialized().unwrap();
    assert_eq!(imported.into_universe().unwrap(), universe);

    let mut invalid = RnnGenome::default();
    invalid.weights[0] = f32::NAN;
    *field(&mut value, &["population", "founder_genome"]) = Value::serialized(&invalid).unwrap();
    let imported: LegacySnapshot = value.deserialized().unwrap();
    assert!(imported.into_universe().is_err());
}
