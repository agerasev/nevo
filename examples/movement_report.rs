//! Replay a saved run from its seed, measure all animals' movement, and verify the
//! final state exactly. Usage: cargo run --release --no-default-features
//! --example movement_report -- runs/RUN/checkpoints/TICK.cbor
use std::{collections::BTreeMap, fs, io::BufReader, path::PathBuf};

use anyhow::{Context, Result, ensure};
use glam::DVec2;
use nevo::{
    ecosystem::{Config, Event, TickReport},
    run::Run,
    universe::Universe,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
struct SavedConfig {
    seed: String,
    ecosystem: Config,
}

#[derive(Deserialize)]
struct Checkpoint {
    format_version: u32,
    world_model: String,
    config: SavedConfig,
    state: Universe,
}

#[derive(Default, Serialize)]
struct Totals {
    ticks: u64,
    animal_ticks: u64,
    active_animal_ticks: u64,
    births: u64,
    deaths: u64,
    immigrants: u64,
    food: f64,
    distance: f64,
    movement_energy: f64,
    upkeep_energy: f64,
    animal_energy: f64,
    plant_energy: f64,
    moving_actions: u64,
    heading_pairs: u64,
    turns_over_90: u64,
    turns_over_150: u64,
    turn_degrees: f64,
}

impl Totals {
    fn merge(&mut self, other: &Self) {
        macro_rules! add { ($($field:ident),*) => { $(self.$field += other.$field;)* }; }
        add!(
            ticks,
            animal_ticks,
            active_animal_ticks,
            births,
            deaths,
            immigrants,
            food,
            distance,
            movement_energy,
            upkeep_energy,
            animal_energy,
            plant_energy,
            moving_actions,
            heading_pairs,
            turns_over_90,
            turns_over_150,
            turn_degrees
        );
    }

    fn summary(&self, plants: usize) -> serde_json::Value {
        fn ratio(a: f64, b: f64) -> Option<f64> {
            (b > 0.0).then(|| a / b)
        }
        json!({
            "totals": self,
            "mean_population": ratio(self.animal_ticks as f64, self.ticks as f64),
            "mean_animal_energy": ratio(self.animal_energy, self.animal_ticks as f64),
            "mean_plant_energy": ratio(self.plant_energy, self.ticks as f64 * plants as f64),
            "food_per_tick": ratio(self.food, self.ticks as f64),
            "birth_death_ratio": ratio(self.births as f64, self.deaths as f64),
            "distance_per_active_tick": ratio(self.distance, self.active_animal_ticks as f64),
            "moving_fraction": ratio(self.moving_actions as f64, self.active_animal_ticks as f64),
            "turns_over_90_fraction": ratio(self.turns_over_90 as f64, self.heading_pairs as f64),
            "turns_over_150_fraction": ratio(self.turns_over_150 as f64, self.heading_pairs as f64),
            "mean_turn_degrees": ratio(self.turn_degrees, self.heading_pairs as f64),
            "food_per_energy_spent": ratio(self.food, self.upkeep_energy + self.movement_energy),
        })
    }
}

fn observe(
    report: &TickReport,
    state: &Universe,
    config: &Config,
    previous: &mut BTreeMap<u64, DVec2>,
) -> Totals {
    let world = state.world();
    let body = config.animal.body;
    let mut sample = Totals {
        ticks: 1,
        animal_ticks: world.animals().len() as u64,
        active_animal_ticks: report.decisions.len() as u64,
        upkeep_energy: world.animals().len() as f64 * body.upkeep,
        animal_energy: world.animals().values().map(|a| a.body.energy).sum(),
        plant_energy: world.plants().iter().map(|p| p.energy).sum(),
        ..Totals::default()
    };
    for event in &report.events {
        match event {
            Event::Birth { .. } => sample.births += 1,
            Event::Death { .. } => sample.deaths += 1,
            Event::Immigrant { .. } => sample.immigrants += 1,
            Event::Meal { gain, .. } => sample.food += gain,
        }
    }
    let mut current = BTreeMap::new();
    for decision in &report.decisions {
        let delta = decision.position_after - decision.position_before;
        let distance = delta.length();
        sample.distance += distance;
        if distance > 0.0 && body.movement_cost > 0.0 {
            let mass = if body.movement_energy_power == 0.0 {
                1.0
            } else {
                decision.energy.powf(body.movement_energy_power)
            };
            sample.movement_energy += mass * (distance * body.movement_cost);
        }
        // Ignore numerical noise; stops reset pairs and newborns have no prior heading.
        if distance <= 1e-9 {
            continue;
        }
        sample.moving_actions += 1;
        let direction = delta / distance;
        if let Some(last) = previous.get(&decision.id.serial()) {
            let dot = last.dot(direction).clamp(-1.0, 1.0);
            sample.heading_pairs += 1;
            sample.turns_over_90 += u64::from(dot < 0.0);
            sample.turns_over_150 += u64::from(dot < -(3.0_f64.sqrt() / 2.0));
            sample.turn_degrees += last.perp_dot(direction).abs().atan2(dot).to_degrees();
        }
        current.insert(decision.id.serial(), direction);
    }
    *previous = current;
    sample
}

fn main() -> Result<()> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .context("provide a checkpoint path")?,
    );
    let checkpoint: Checkpoint = ciborium::from_reader(BufReader::new(fs::File::open(&path)?))?;
    ensure!(
        checkpoint.format_version == 12 && checkpoint.world_model == "foraging-sense-first-v7",
        "unsupported checkpoint; use the matching simulation build"
    );
    checkpoint.state.validate()?;
    let seed: u64 = checkpoint.config.seed.parse()?;
    let ticks = checkpoint.state.step();
    ensure!(ticks > 0, "a nonempty run is required");
    let config = checkpoint.config.ecosystem;
    let mut run = Run::new(config.clone(), seed)?;
    let mut total = Totals::default();
    let mut late = Totals::default();
    let mut window = Totals::default();
    let mut previous = BTreeMap::new();
    let mut windows = Vec::new();
    let late_start = ticks - (ticks / 3).max(1);
    for tick in 1..=ticks {
        let report = run.tick()?;
        let state = run.universe().read();
        let sample = observe(&report, &state, &config, &mut previous);
        total.merge(&sample);
        window.merge(&sample);
        if tick > late_start {
            late.merge(&sample);
        }
        if tick % 1000 == 0 || tick == ticks {
            windows.push(json!({"end_tick": tick, "summary": window.summary(config.plants)}));
            window = Totals::default();
        }
    }
    ensure!(
        *run.universe().read() == checkpoint.state,
        "replay differs from the saved state; do not interpret this report"
    );
    let output = path
        .parent()
        .and_then(|p| p.parent())
        .context("expected run/checkpoints/file")?
        .join("analysis");
    fs::create_dir_all(&output)?;
    fs::write(
        output.join("movement.json"),
        serde_json::to_vec_pretty(&json!({
            "checkpoint": path, "seed": seed.to_string(), "ticks": ticks,
            "code_revision": env!("NEVO_REVISION"), "exact_final_state_verified": true,
            "definition": "Angles between consecutive controlled displacements of the same animal, both longer than 1e-9; stops break pairs; birth offsets excluded.",
            "late_start_tick": late_start, "late_end_tick": ticks,
            "overall": total.summary(config.plants), "late": late.summary(config.plants),
            "windows": windows,
        }))?,
    )?;
    println!(
        "Verified {ticks} replayed ticks; wrote {}",
        output.join("movement.json").display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_pairs_count_reversals_and_reset_after_stops() {
        let mut config = Config {
            initial_animals: 1,
            plants: 0,
            minimum_population: 0,
            ..Config::default()
        };
        config.animal.body.movement_cost = 0.00045;
        config.animal.body.movement_energy_power = 0.0;
        let mut state = Universe::new(config.clone(), 1).unwrap();
        let mut previous = BTreeMap::new();
        let mut total = Totals::default();
        for delta in [DVec2::X, DVec2::X, -DVec2::X, DVec2::ZERO, DVec2::Y] {
            let mut report = state.advance().unwrap();
            report.decisions[0].position_after = report.decisions[0].position_before + delta;
            total.merge(&observe(&report, &state, &config, &mut previous));
        }
        assert_eq!(total.moving_actions, 4);
        assert_eq!(total.heading_pairs, 2);
        assert_eq!(total.turns_over_90, 1);
        assert_eq!(total.turns_over_150, 1);
        assert_eq!(total.turn_degrees, 180.0);
        assert_eq!(total.distance, 4.0);
        assert!((total.movement_energy - 0.0018).abs() < 1e-12);
    }
}
