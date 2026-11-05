//! Read-only metrics browsing, independent of the simulated world.

mod history;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use egui_plot::{HoverPosition, Legend, Line, MarkerShape, Plot, Points};
use wgame_egui::egui;

use history::{History, Segment};

#[derive(Debug, PartialEq)]
struct PlotQuery {
    metric: String,
    range: Option<[f64; 2]>,
    pixels: usize,
}

pub(super) struct Panel {
    path: Option<PathBuf>,
    history: Option<History>,
    segments: Vec<Segment>,
    plotted: Option<PlotQuery>,
    next_poll: Instant,
    loading: bool,
    error: Option<String>,
    filter: String,
    selected: String,
    follow: bool,
    range: Option<[f64; 2]>,
    pixels: usize,
    fit_when_ready: bool,
    reset_plot: bool,
    #[cfg(test)]
    plot_updates: usize,
    #[cfg(test)]
    plot_rect: Option<egui::Rect>,
    #[cfg(test)]
    plot_bounds: Option<[f64; 2]>,
}

impl Panel {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            history: path.clone().map(History::new),
            path,
            segments: Vec::new(),
            plotted: None,
            next_poll: Instant::now(),
            loading: false,
            error: None,
            filter: String::new(),
            selected: String::new(),
            follow: true,
            range: None,
            pixels: 1024,
            fit_when_ready: true,
            reset_plot: false,
            #[cfg(test)]
            plot_updates: 0,
            #[cfg(test)]
            plot_rect: None,
            #[cfg(test)]
            plot_bounds: None,
        }
    }

    /// Runs once per viewer loop, outside repeatable UI layout and world locks.
    /// Catch up one bounded batch per frame; at EOF, check for appends periodically.
    pub fn poll(&mut self) {
        let Some(history) = &mut self.history else {
            return;
        };
        let mut changed = false;
        let now = Instant::now();
        if now >= self.next_poll {
            self.next_poll = now + Duration::from_millis(200);
            let windows = history.total_windows();
            match history.poll() {
                Ok(received) => {
                    self.error = None;
                    self.loading = received;
                    changed = history.total_windows() != windows;
                    if received {
                        self.next_poll = now;
                    }
                }
                Err(error) => {
                    self.error = Some(format!("{error:#}"));
                    self.loading = false;
                }
            }
        }
        if self.selected.is_empty() {
            self.selected = history.names().next().unwrap_or_default().to_owned();
        }
        let query = PlotQuery {
            metric: self.selected.clone(),
            range: self.range,
            pixels: self.pixels,
        };
        if changed || self.plotted.as_ref() != Some(&query) {
            self.segments = history.query(&query.metric, query.range, query.pixels);
            #[cfg(test)]
            {
                self.plot_updates += 1;
            }
            if query.range.is_none() && (self.follow || self.fit_when_ready) {
                self.reset_plot = true;
                self.fit_when_ready = false;
            }
            self.plotted = Some(query);
        }
    }

    fn fit(&mut self) {
        self.range = None;
        self.fit_when_ready = true;
        // Fitting the already computed full view needs no further work.
        if self
            .plotted
            .as_ref()
            .is_some_and(|query| query.metric == self.selected && query.range.is_none())
        {
            self.reset_plot = true;
            self.fit_when_ready = false;
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let (Some(path), Some(history)) = (&self.path, &self.history) else {
            ui.label("No metrics file is attached to this viewer.");
            return;
        };
        ui.horizontal(|ui| {
            ui.heading("Metrics");
            ui.weak(path.display().to_string());
        });
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} loaded windows", history.total_windows()));
            if let Some(tick) = history.last_tick() {
                ui.label(format!("Latest recorded tick {tick}"));
            }
            if self.loading {
                ui.spinner();
                ui.weak("Loading history…");
            } else if history.has_pending_record() {
                ui.weak("Waiting for the remainder of a record…");
            }
        });
        if history.total_windows() == 0 {
            ui.add_space(12.0);
            ui.label("Waiting for recorded metric windows…");
            ui.weak(
                "Enable collection with configs/metrics.toml. New records appear automatically.",
            );
            return;
        }
        let mut fit = false;
        ui.horizontal(|ui| {
            ui.label("Find metric");
            ui.text_edit_singleline(&mut self.filter);
            if ui.checkbox(&mut self.follow, "Follow data").changed() && self.follow {
                fit = true;
            }
            fit |= ui.button("Fit all").clicked();
        });
        let filter = self.filter.to_lowercase();
        egui::ComboBox::from_id_salt("metric-name")
            .width(ui.available_width())
            .selected_text(if self.selected.is_empty() {
                "Choose a metric"
            } else {
                &self.selected
            })
            .show_ui(ui, |ui| {
                for name in history
                    .names()
                    .filter(|name| name.to_lowercase().contains(&filter))
                {
                    if ui
                        .selectable_value(&mut self.selected, name.to_owned(), name)
                        .changed()
                    {
                        fit = true;
                    }
                }
            });
        if fit {
            self.fit();
        }
        if self.selected.is_empty() {
            ui.label("No scalar metrics are present in the recorded summaries.");
            return;
        }
        ui.weak(
            "Drag/scroll to pan · Ctrl+scroll or pinch to zoom · double-click to fit all · diamonds mark partial windows",
        );
        let loading = self
            .plotted
            .as_ref()
            .is_none_or(|query| query.metric != self.selected || query.range != self.range)
            || self.fit_when_ready;
        if loading {
            ui.weak("Updating plot…");
        } else {
            ui.weak("Zoomed-out curves preserve minimum and maximum values.");
        }
        // A previous range may remain visible while its replacement is computed, but
        // values from a different metric must never appear under the new name.
        let segments = if self
            .plotted
            .as_ref()
            .is_some_and(|query| query.metric == self.selected)
        {
            self.segments.as_slice()
        } else {
            &[]
        };
        let reset = std::mem::take(&mut self.reset_plot);
        let mut plot = Plot::new(("metrics-plot", &self.selected))
            .legend(Legend::default())
            .allow_double_click_reset(false)
            .x_axis_label("Tick")
            .y_axis_label(&self.selected)
            .height(ui.available_height().max(120.0))
            .label_formatter(|position| match position {
                HoverPosition::NearDataPoint {
                    plot_name,
                    position,
                    ..
                } => {
                    let found = segments
                        .iter()
                        .filter(|segment| plot_name.starts_with(&segment.name))
                        .flat_map(|segment| &segment.points)
                        .find(|point| point.tick as f64 == position.x);
                    let mut text = format!(
                        "{plot_name}\nTick {:.0}\nValue {:.6}",
                        position.x, position.y
                    );
                    if let Some(window) = found.and_then(|point| point.window) {
                        text.push_str(&format!(
                            "\nWindow ({}, {}] · {} ticks{}",
                            window.start_tick,
                            window.end_tick,
                            window.samples,
                            if window.complete { "" } else { " · partial" }
                        ));
                    }
                    Some(text)
                }
                HoverPosition::Elsewhere { position } => {
                    Some(format!("Tick {:.0}\nValue {:.6}", position.x, position.y))
                }
            });
        if reset {
            plot = plot.reset();
        }
        // Fitting/following uses the full log extent, even if this metric
        // disappeared from newer windows after a configuration change.
        if (reset || self.follow)
            && let Some([min, max]) = self.history.as_ref().and_then(History::extent)
        {
            plot = plot.include_x(min as f64).include_x(max as f64);
        }
        let response = plot.show(ui, |plot_ui| {
            // Once browsing, replacement data cannot alter the user's bounds.
            // Interaction is applied by egui_plot after this setting.
            plot_ui.set_auto_bounds(reset || (self.follow && !self.fit_when_ready));
            for segment in segments {
                let color = session_color(segment.session);
                let points: Vec<_> = segment
                    .points
                    .iter()
                    .map(|point| [point.tick as f64, point.value])
                    .collect();
                if points.len() > 1 {
                    plot_ui.line(Line::new(&segment.name, points).color(color));
                } else {
                    plot_ui.points(Points::new(&segment.name, points).color(color).radius(3.0));
                }
                let partial: Vec<_> = segment
                    .points
                    .iter()
                    .filter(|point| point.window.is_some_and(|window| !window.complete))
                    .map(|point| [point.tick as f64, point.value])
                    .collect();
                if !partial.is_empty() {
                    plot_ui.points(
                        Points::new(format!("{} · partial", segment.name), partial)
                            .color(color)
                            .shape(MarkerShape::Diamond)
                            .radius(5.0),
                    );
                }
            }
        });
        #[cfg(test)]
        {
            self.plot_rect = Some(*response.transform.frame());
            let bounds = response.transform.bounds();
            self.plot_bounds = Some([bounds.min()[0], bounds.max()[0]]);
        }
        let interacted = response.response.dragged()
            || (response.response.hovered()
                && ui.input(|input| {
                    input.smooth_scroll_delta != egui::Vec2::ZERO || input.zoom_delta() != 1.0
                }));
        if interacted {
            self.follow = false;
            self.fit_when_ready = false;
        }
        self.pixels = response.transform.frame().width().ceil().max(1.0) as usize;
        if response.response.double_clicked() {
            self.fit();
        } else if !self.follow && !self.fit_when_ready {
            let bounds = response.transform.bounds();
            self.range = Some([bounds.min()[0], bounds.max()[0]]);
        }
    }
}

fn session_color(session: u64) -> egui::Color32 {
    const COLORS: [egui::Color32; 6] = [
        egui::Color32::from_rgb(93, 173, 226),
        egui::Color32::from_rgb(245, 176, 65),
        egui::Color32::from_rgb(88, 214, 141),
        egui::Color32::from_rgb(195, 155, 211),
        egui::Color32::from_rgb(236, 112, 99),
        egui::Color32::from_rgb(72, 201, 176),
    ];
    COLORS[session.wrapping_sub(1) as usize % COLORS.len()]
}

#[cfg(test)]
mod tests;
