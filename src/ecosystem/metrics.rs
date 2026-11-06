//! Inexpensive observations from tick updates and population snapshots.

use serde::Serialize;

use super::*;
use crate::metrics::Metrics;

/// Tick interval (start_tick, end_tick]; exposure is metadata, not a plotted metric.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Window {
    pub start_tick: u64,
    pub end_tick: u64,
    pub samples: u64,
    pub active_animal_ticks: u64,
    pub complete: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Cache {
    pub config: MetricsConfig,
    pub window: Option<Window>,
    births: u64,
    deaths: u64,
    immigrants: u64,
    food: f64,
    plant_growth: f64,
    plant_recruitment_attempts: u64,
    plant_recruits: u64,
    distance: f64,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            config: MetricsConfig {
                enabled: false,
                ..MetricsConfig::default()
            },
            window: None,
            births: 0,
            deaths: 0,
            immigrants: 0,
            food: 0.0,
            plant_growth: 0.0,
            plant_recruitment_attempts: 0,
            plant_recruits: 0,
            distance: 0.0,
        }
    }
}

impl Cache {
    pub fn new<MC, IC, P>(config: &Config<MC, IC, P>) -> Self {
        Self {
            config: config.metrics,
            ..Self::default()
        }
    }

    pub fn begin(&self, tick: u64) -> Self {
        if !self.config.enabled {
            return self.clone();
        }
        let mut next = if self.window.is_none_or(|window| window.complete) {
            Self {
                config: self.config,
                window: Some(Window {
                    start_tick: tick.saturating_sub(1),
                    ..Window::default()
                }),
                ..Self::default()
            }
        } else {
            self.clone()
        };
        let window = next.window.as_mut().expect("enabled metrics have a window");
        window.end_tick = tick;
        window.samples += 1;
        window.complete = window.samples >= self.config.window_ticks;
        next
    }

    /// Actual plant energy gained after capping, excluding admission energy.
    pub(super) fn observe_plant_growth(&mut self, energy: f64) {
        if self.window.is_some() {
            self.plant_growth += energy;
        }
    }

    pub(super) fn observe_replenishment(&mut self, attempts: usize, recruits: usize) {
        if self.window.is_some() {
            self.plant_recruitment_attempts += attempts as u64;
            self.plant_recruits += recruits as u64;
        }
    }

    pub fn observe(&mut self, report: &TickReport) {
        let Some(window) = &mut self.window else {
            return;
        };
        window.active_animal_ticks += report.decisions.len() as u64;
        for decision in &report.decisions {
            // Includes actors that subsequently die; excludes birth/growth displacement.
            self.distance += decision.position_before.distance(decision.position_after);
        }
        for event in &report.events {
            match event {
                Event::Meal { gain, .. } => self.food += gain,
                Event::Birth { .. } => self.births += 1,
                Event::Death { .. } => self.deaths += 1,
                Event::Immigrant { .. } => self.immigrants += 1,
            }
        }
    }
}

/// Population and energy are endpoint snapshots; all activity is expressed as rates.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Summary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub population: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plant_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_animal_energy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_plant_energy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub food_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plant_growth_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plant_recruitment_attempts_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plant_recruits_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub births_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deaths_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immigrants_per_tick: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_per_animal_tick: Option<f64>,
}

impl<M, G, P, MC, IC> Ecosystem<M, G, P, MC, IC> {
    pub fn metrics_enabled(&self) -> bool {
        self.metrics.config.enabled
    }
    pub fn metrics_window(&self) -> Option<Window> {
        self.metrics.window
    }
}

impl<M, G, P, MC, IC> Metrics for Ecosystem<M, G, P, MC, IC> {
    type Summary = Summary;

    fn collect_metrics(&self, summary: &mut Summary) {
        *summary = Summary::default();
        if !self.metrics_enabled() {
            return;
        }
        let cache = &self.metrics;
        summary.window = cache.window;
        summary.population = Some(self.animals.len() as u64);
        summary.plant_count = Some(self.plants.len() as u64);
        summary.mean_animal_energy = (!self.animals.is_empty()).then(|| {
            self.animals.values().map(|a| a.body.energy).sum::<f64>() / self.animals.len() as f64
        });
        summary.mean_plant_energy = (!self.plants.is_empty())
            .then(|| self.plants.iter().map(|p| p.energy).sum::<f64>() / self.plants.len() as f64);
        if let Some(window) = cache.window {
            let ticks = window.samples as f64;
            summary.food_per_tick = Some(cache.food / ticks);
            summary.plant_growth_per_tick = Some(cache.plant_growth / ticks);
            if self.parameters.plant.replenishment.is_some() {
                summary.plant_recruitment_attempts_per_tick =
                    Some(cache.plant_recruitment_attempts as f64 / ticks);
                summary.plant_recruits_per_tick = Some(cache.plant_recruits as f64 / ticks);
            }
            summary.births_per_tick = Some(cache.births as f64 / ticks);
            summary.deaths_per_tick = Some(cache.deaths as f64 / ticks);
            summary.immigrants_per_tick = Some(cache.immigrants as f64 / ticks);
            summary.distance_per_animal_tick = (window.active_animal_ticks > 0)
                .then(|| cache.distance / window.active_animal_ticks as f64);
        }
    }
}
