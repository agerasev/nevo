//! Frozen-controller common-garden assay against the archived b86d5bd library.
//! Run through compare_controllers.py; no changes to the simulation rules.
use anyhow::{Result, ensure};
use nevo::{
    context::Context,
    ecosystem::{Config, Ecosystem, Event, FoodObservation, MoveAction, TickReport},
    mind::{
        Mind,
        animal::AnimalMind,
        rnn::{RnnGenome, WeightMutation},
    },
    universe::Simulation,
};
use rand::{SeedableRng, seq::SliceRandom};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct GradientFollower(AnimalMind);
impl From<RnnGenome> for GradientFollower {
    fn from(g: RnnGenome) -> Self {
        Self(AnimalMind::from(g))
    }
}
impl AsRef<RnnGenome> for GradientFollower {
    fn as_ref(&self) -> &RnnGenome {
        self.0.as_ref()
    }
}
impl<C: Context + ?Sized> Mind<C> for GradientFollower {
    type Input = FoodObservation;
    type Output = MoveAction;
    type Update = ();
    type Config = <AnimalMind as Mind<C>>::Config;
    fn compute_update(&self, input: Self::Input, _: &mut C) -> Result<(Self::Output, ())> {
        Ok((
            MoveAction {
                direction: input.gradient,
                speed_fraction: 1.0,
            },
            (),
        ))
    }
    fn apply_update(&mut self, _: ()) {}
    fn validate(&self) -> Result<()> {
        <AnimalMind as Mind<C>>::validate(&self.0)
    }
}

#[derive(Clone, Default, Serialize)]
struct Totals {
    ticks: u64,
    animal_ticks: u64,
    active_animal_ticks: u64,
    births: u64,
    deaths: u64,
    immigrants: u64,
    meals: u64,
    food_gained: f64,
    distance: f64,
    aligned_actions: u64,
    moving_actions: u64,
}
impl Totals {
    fn observe(&mut self, report: &TickReport, population: usize) {
        self.ticks += 1;
        self.animal_ticks += population as u64;
        self.active_animal_ticks += report.decisions.len() as u64;
        for e in &report.events {
            match e {
                Event::Birth { .. } => self.births += 1,
                Event::Death { .. } => self.deaths += 1,
                Event::Immigrant { .. } => self.immigrants += 1,
                Event::Meal { gain, .. } => {
                    self.meals += 1;
                    self.food_gained += gain;
                }
            }
        }
        for d in &report.decisions {
            let displacement = d.position_after - d.position_before;
            self.distance += displacement.length();
            if displacement.length() > 0.0 {
                self.moving_actions += 1;
                if displacement.dot(d.observation.gradient) > 0.0 {
                    self.aligned_actions += 1;
                }
            }
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 5,
        "usage: compare_controllers CHECKPOINT SEED TICKS OUTPUT_DIR"
    );
    let seed: u64 = args[2].parse()?;
    let ticks: u64 = args[3].parse()?;
    ensure!(
        ticks > 0 && ticks % 2 == 0,
        "positive even tick budget required"
    );
    let out = PathBuf::from(&args[4]);
    std::fs::create_dir_all(&out)?;
    let input = BufReader::new(File::open(&args[1])?);
    let checkpoint: Value = if args[1].ends_with(".json") {
        serde_json::from_reader(input)?
    } else {
        let value: ciborium::Value = ciborium::from_reader(input)?;
        serde_json::to_value(value)?
    };
    ensure!(
        checkpoint["format_version"] == 7,
        "expected the original version-7 checkpoint"
    );
    ensure!(
        checkpoint["agent_model"] == "animal-rnn-3-4-3-v1",
        "unexpected saved mind model"
    );
    let saved = &checkpoint["state"]["world"]["animals"];
    let mut genomes: Vec<(u64, RnnGenome)> = saved
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, a)| {
            Ok((
                id.parse()?,
                serde_json::from_value(a["mind"]["brain"]["genome"].clone())?,
            ))
        })
        .collect::<Result<_>>()?;
    genomes.sort_by_key(|x| x.0);
    genomes.shuffle(&mut Xoshiro256PlusPlus::seed_from_u64(
        seed ^ 0x738d6f2a9415bc07,
    ));
    let mut config = Config::default();
    config.initial_animals = genomes.len();
    config.minimum_population = 0;
    config.metrics.enabled = false;
    let base: Ecosystem = Ecosystem::with_genetics(
        config.clone(),
        seed,
        Some(RnnGenome::default()),
        WeightMutation { max_delta: 0.0 },
    )?;
    let mut initial = serde_json::to_value(base)?;
    let mut assignments = BTreeMap::new();
    for (i, (source_id, genome)) in genomes.iter().enumerate() {
        let id = (i + 1).to_string();
        initial["animals"][&id]["mind"]["brain"]["genome"] = serde_json::to_value(genome)?;
        initial["animals"][&id]["mind"]["brain"]["memory"] = json!([0., 0., 0., 0.]);
        assignments.insert(id, *source_id);
    }
    let mut evolved: Ecosystem = serde_json::from_value(initial.clone())?;
    let mut gradient: Ecosystem<GradientFollower> = serde_json::from_value(initial.clone())?;
    evolved.configure(&config)?;
    gradient.configure(&config)?;
    evolved.validate()?;
    gradient.validate()?;
    ensure!(
        serde_json::to_value(&evolved)? == serde_json::to_value(&gradient)?,
        "initial worlds differ"
    );
    ensure!(
        evolved.minimum_population() == 0 && gradient.minimum_population() == 0,
        "immigration enabled"
    );
    ensure!(
        evolved
            .animals()
            .values()
            .all(|a| a.body.energy == 100.0 && a.mind.brain.memory == [0.; 4]),
        "founder normalization failed"
    );
    ensure!(
        evolved.plants().len() == 500 && evolved.plants().iter().all(|p| p.energy == 400.0),
        "initial plants differ"
    );
    let allowed: BTreeSet<String> = genomes
        .iter()
        .map(|(_, g)| serde_json::to_string(g).unwrap())
        .collect();
    let mut initial_file = BufWriter::new(File::create(out.join("initial.cbor"))?);
    ciborium::into_writer(&initial, &mut initial_file)?;
    initial_file.flush()?;
    let mut log = BufWriter::new(File::create(out.join("windows.jsonl"))?);
    let mut totals = [Totals::default(), Totals::default()];
    let mut late = [Totals::default(), Totals::default()];
    let mut window = [Totals::default(), Totals::default()];
    let mut extinction: [Option<u64>; 2] = [None, None];
    for tick in 1..=ticks {
        let reports = [evolved.advance(tick)?, gradient.advance(tick)?];
        let populations = [evolved.animals().len(), gradient.animals().len()];
        for i in 0..2 {
            totals[i].observe(&reports[i], populations[i]);
            window[i].observe(&reports[i], populations[i]);
            if tick > ticks / 2 {
                late[i].observe(&reports[i], populations[i]);
            }
            if populations[i] == 0 && extinction[i].is_none() {
                extinction[i] = Some(tick);
            }
            ensure!(
                populations[i] as i64
                    == genomes.len() as i64 + totals[i].births as i64 - totals[i].deaths as i64,
                "population accounting failed"
            );
            ensure!(totals[i].immigrants == 0, "unexpected immigrant");
        }
        if tick % 100 == 0 || tick == ticks {
            for i in 0..2 {
                let (energy, plant_energy) = if i == 0 {
                    (
                        evolved
                            .animals()
                            .values()
                            .map(|a| a.body.energy)
                            .sum::<f64>(),
                        evolved.plants().iter().map(|p| p.energy).sum::<f64>(),
                    )
                } else {
                    (
                        gradient
                            .animals()
                            .values()
                            .map(|a| a.body.energy)
                            .sum::<f64>(),
                        gradient.plants().iter().map(|p| p.energy).sum::<f64>(),
                    )
                };
                serde_json::to_writer(
                    &mut log,
                    &json!({"controller":(["evolved_rnn","gradient_max"][i]),"tick":tick,"population":populations[i],"animal_energy":energy,"plant_energy":plant_energy,"events":window[i]}),
                )?;
                writeln!(log)?;
                window[i] = Totals::default();
            }
        }
    }
    log.flush()?;
    for a in evolved.animals().values() {
        ensure!(
            allowed.contains(&serde_json::to_string(&a.mind.brain.genome)?),
            "genome mutated"
        );
    }
    for (name, state) in [
        ("evolved_rnn", serde_json::to_value(&evolved)?),
        ("gradient_max", serde_json::to_value(&gradient)?),
    ] {
        let mut file = BufWriter::new(File::create(out.join(format!("{name}-final.cbor")))?);
        ciborium::into_writer(&json!({"tick":ticks,"world":state}), &mut file)?;
        file.flush()?;
    }
    let summary = json!({"seed":seed,"ticks":ticks,"simulation_revision":"b86d5bd","founders":genomes.len(),"source_genome_assignments":assignments,
        "initial_worlds_equal":true,"initial_energy":100.,"plants":500,"mutation_delta":0.,"immigration_floor":0,
        "evolved_rnn":{"total":totals[0],"late":late[0],"extinction_tick":extinction[0],"final_population":evolved.animals().len()},
        "gradient_max":{"total":totals[1],"late":late[1],"extinction_tick":extinction[1],"final_population":gradient.animals().len()}});
    serde_json::to_writer_pretty(
        BufWriter::new(File::create(out.join("summary.json"))?),
        &summary,
    )?;
    println!(
        "seed {seed}: evolved final={} gradient final={}",
        evolved.animals().len(),
        gradient.animals().len()
    );
    Ok(())
}
