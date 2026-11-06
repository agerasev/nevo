//! Typed, nonpersistent observations of the current population and recent ticks.

use serde::{Serialize, Serializer};

use super::*;
use crate::metrics::{Metrics, SelectedStats, Selection, Statistics};

/// Tick interval `(start_tick, end_tick]`; samples are actual completed ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Window {
    pub start_tick: u64,
    pub end_tick: u64,
    pub samples: u64,
    pub complete: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PlantCache {
    pub energy: Selection,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BodyCache {
    pub body: BodyMetricsConfig,
    pub animal: AnimalMetricsConfig,
    pub food_gained: f64,
    pub offspring: u64,
    pub distance: f64,
    pub active_ticks: u64,
}
impl Default for BodyCache {
    fn default() -> Self {
        Self {
            body: BodyMetricsConfig {
                energy: Selection::default(),
            },
            animal: AnimalMetricsConfig::default(),
            food_gained: 0.0,
            offspring: 0,
            distance: 0.0,
            active_ticks: 0,
        }
    }
}
impl BodyCache {
    pub fn new(body: BodyMetricsConfig, animal: AnimalMetricsConfig) -> Self {
        Self {
            body,
            animal,
            ..Self::default()
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new(self.body, self.animal);
    }
    pub fn observe_action(&mut self, distance: f64) {
        if self.animal.distance.enabled() {
            self.distance += distance;
        }
        if self.animal.observed_survival.enabled() {
            self.active_ticks += 1;
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Cache {
    pub config: MetricsConfig,
    pub animal_config: AnimalMetricsConfig,
    pub body_config: BodyMetricsConfig,
    pub plant_config: PlantMetricsConfig,
    pub window: Option<Window>,
    pub meals: u64,
    pub births: u64,
    pub deaths: u64,
    pub immigrants: u64,
    pub food_gained: f64,
    pub active_animal_ticks: u64,
    pub observed_survival: SelectedStats,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            config: MetricsConfig {
                enabled: false,
                population: false,
                event_rates: false,
                genetic_diversity: false,
                window_ticks: 1000,
            },
            animal_config: AnimalMetricsConfig::default(),
            body_config: BodyMetricsConfig {
                energy: Selection::default(),
            },
            plant_config: PlantMetricsConfig {
                energy: Selection::default(),
            },
            window: None,
            meals: 0,
            births: 0,
            deaths: 0,
            immigrants: 0,
            food_gained: 0.0,
            active_animal_ticks: 0,
            observed_survival: SelectedStats::default(),
        }
    }
}
impl Cache {
    pub fn new<MC, IC, P>(config: &Config<MC, IC, P>) -> Self {
        if !config.metrics.enabled {
            return Self::default();
        }
        Self {
            config: config.metrics,
            animal_config: config.animal.metrics,
            body_config: config.animal.body.metrics,
            plant_config: config.plant.metrics,
            observed_survival: SelectedStats::new(config.animal.metrics.observed_survival),
            ..Self::default()
        }
    }

    /// Start a fresh window without changing the configured collectors.
    pub fn begin(&self, tick: u64) -> (Self, bool) {
        if !self.config.enabled {
            return (self.clone(), false);
        }
        let reset = self.window.is_none_or(|window| window.complete);
        let mut next = if reset {
            Self {
                config: self.config,
                animal_config: self.animal_config,
                body_config: self.body_config,
                plant_config: self.plant_config,
                window: Some(Window {
                    start_tick: tick.saturating_sub(1),
                    end_tick: tick.saturating_sub(1),
                    ..Window::default()
                }),
                observed_survival: SelectedStats::new(self.animal_config.observed_survival),
                ..Self::default()
            }
        } else {
            self.clone()
        };
        let window = next.window.as_mut().expect("enabled metrics have a window");
        window.end_tick = tick;
        window.samples += 1;
        window.complete = window.samples >= self.config.window_ticks;
        (next, reset)
    }
    pub fn observe(&mut self, report: &TickReport) {
        if !self.config.event_rates {
            return;
        }
        self.active_animal_ticks += report.decisions.len() as u64;
        for event in &report.events {
            match event {
                Event::Meal { gain, .. } => {
                    self.meals += 1;
                    self.food_gained += gain;
                }
                Event::Birth { .. } => self.births += 1,
                Event::Death { .. } => self.deaths += 1,
                Event::Immigrant { .. } => self.immigrants += 1,
            }
        }
    }
}

/// Snapshot energies and recent totals for currently present animals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BodySummary {
    pub energy: SelectedStats,
    pub food_gained: SelectedStats,
    pub offspring: SelectedStats,
    pub distance: SelectedStats,
}
impl Serialize for BodySummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Values {
            #[serde(skip_serializing_if = "Option::is_none")]
            energy: Option<Statistics>,
            #[serde(skip_serializing_if = "Option::is_none")]
            food_gained: Option<Statistics>,
            #[serde(skip_serializing_if = "Option::is_none")]
            offspring: Option<Statistics>,
            #[serde(skip_serializing_if = "Option::is_none")]
            distance: Option<Statistics>,
        }
        Values {
            energy: selected(&self.energy),
            food_gained: selected(&self.food_gained),
            offspring: selected(&self.offspring),
            distance: selected(&self.distance),
        }
        .serialize(serializer)
    }
}
fn selected(stats: &SelectedStats) -> Option<Statistics> {
    stats.enabled().then(|| stats.value())
}
fn sample(summary: &mut SelectedStats, selection: Selection, value: f64) {
    if selection.enabled() {
        let mut stats = SelectedStats::new(selection);
        stats.push(value);
        summary.merge(&stats);
    }
}
impl Metrics for Body {
    type Summary = BodySummary;
    fn collect_metrics(&self, summary: &mut Self::Summary) {
        sample(&mut summary.energy, self.metrics.body.energy, self.energy);
        sample(
            &mut summary.food_gained,
            self.metrics.animal.food_gained,
            self.metrics.food_gained,
        );
        sample(
            &mut summary.offspring,
            self.metrics.animal.offspring,
            self.metrics.offspring as f64,
        );
        sample(
            &mut summary.distance,
            self.metrics.animal.distance,
            self.metrics.distance,
        );
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantSummary {
    pub energy: SelectedStats,
}
impl Serialize for PlantSummary {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Values {
            #[serde(skip_serializing_if = "Option::is_none")]
            energy: Option<Statistics>,
        }
        Values {
            energy: selected(&self.energy),
        }
        .serialize(serializer)
    }
}
impl Metrics for Plant {
    type Summary = PlantSummary;
    fn collect_metrics(&self, summary: &mut Self::Summary) {
        sample(&mut summary.energy, self.metrics.energy, self.energy);
    }
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct AnimalSummary<B, M> {
    pub body: B,
    pub mind: M,
}
impl<B: Metrics, M: Metrics> Metrics for Animal<B, M> {
    type Summary = AnimalSummary<B::Summary, M::Summary>;
    fn collect_metrics(&self, summary: &mut Self::Summary) {
        self.body.collect_metrics(&mut summary.body);
        self.mind.collect_metrics(&mut summary.mind);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PopulationSummary {
    pub animals: u64,
    pub plants: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct EventSummary {
    pub meals: u64,
    pub births: u64,
    pub deaths: u64,
    pub immigrants: u64,
    pub food_gained: f64,
    /// Exposure of animals that actually sensed and acted, excluding new arrivals.
    pub active_animal_ticks: u64,
    pub births_per_tick: f64,
    pub deaths_per_tick: f64,
    pub immigrants_per_tick: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meals_per_animal_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub food_per_animal_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub births_per_animal_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deaths_per_animal_tick: Option<f64>,
}

/// A snapshot of one ecosystem and one window. Unlike leaf accumulators,
/// ecosystem collection replaces this value; independent windows are not pooled.
#[derive(Default, Serialize)]
pub struct Summary<M: Default, G: Default> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub population: Option<PopulationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<EventSummary>,
    pub animal: AnimalSummary<BodySummary, M>,
    pub plant: PlantSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_survival: Option<Statistics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genome: Option<G>,
}
impl<M, G, P, MC, IC> Ecosystem<M, G, P, MC, IC> {
    pub fn metrics_enabled(&self) -> bool {
        self.metrics.config.enabled
    }
    pub fn metrics_window(&self) -> Option<Window> {
        self.metrics.window
    }
}
impl<M: Metrics + AsRef<G>, G: Metrics, P, MC, IC> Metrics for Ecosystem<M, G, P, MC, IC> {
    type Summary = Summary<M::Summary, G::Summary>;
    fn collect_metrics(&self, summary: &mut Self::Summary) {
        *summary = Self::Summary::default();
        if !self.metrics_enabled() {
            return;
        }
        let cache = &self.metrics;
        summary.window = cache.window;
        if cache.config.population {
            summary.population = Some(PopulationSummary {
                animals: self.animals.len() as u64,
                plants: self.plants.len() as u64,
            });
        }
        if cache.config.event_rates
            && let Some(window) = cache.window
        {
            let ticks = window.samples as f64;
            let exposure = cache.active_animal_ticks as f64;
            summary.events = Some(EventSummary {
                meals: cache.meals,
                births: cache.births,
                deaths: cache.deaths,
                immigrants: cache.immigrants,
                food_gained: cache.food_gained,
                active_animal_ticks: cache.active_animal_ticks,
                births_per_tick: cache.births as f64 / ticks,
                deaths_per_tick: cache.deaths as f64 / ticks,
                immigrants_per_tick: cache.immigrants as f64 / ticks,
                meals_per_animal_tick: (exposure > 0.0).then(|| cache.meals as f64 / exposure),
                food_per_animal_tick: (exposure > 0.0).then(|| cache.food_gained / exposure),
                births_per_animal_tick: (exposure > 0.0).then(|| cache.births as f64 / exposure),
                deaths_per_animal_tick: (exposure > 0.0).then(|| cache.deaths as f64 / exposure),
            });
        }
        // Initialize selected reducers even when the population is empty.
        summary.animal.body = BodySummary {
            energy: SelectedStats::new(cache.body_config.energy),
            food_gained: SelectedStats::new(cache.animal_config.food_gained),
            offspring: SelectedStats::new(cache.animal_config.offspring),
            distance: SelectedStats::new(cache.animal_config.distance),
        };
        summary.plant.energy = SelectedStats::new(cache.plant_config.energy);
        for animal in self.animals.values() {
            animal.collect_metrics(&mut summary.animal);
        }
        for plant in &self.plants {
            plant.collect_metrics(&mut summary.plant);
        }
        summary.observed_survival = selected(&cache.observed_survival);
        if cache.config.genetic_diversity {
            let genomes = summary.genome.get_or_insert_with(Default::default);
            for animal in self.animals.values() {
                animal.mind.as_ref().collect_metrics(genomes);
            }
        }
    }
}
