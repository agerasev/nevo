//! Read-only metrics browsing, independent of the simulated world.

mod history;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use egui_plot::{HoverPosition, Legend, Line, MarkerShape, Plot, Points};
use wgame_egui::egui;

use crate::metrics::log::Reader;
use history::{History, WINDOW_LIMIT};

pub(super) struct Panel {
    path: Option<PathBuf>,
    reader: Option<Reader>,
    history: History,
    next_poll: Instant,
    error: Option<String>,
    filter: String,
    selected: String,
    follow: bool,
    reset_plot: bool,
}

impl Panel {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            reader: path.as_ref().map(Reader::new),
            path,
            history: History::default(),
            next_poll: Instant::now(),
            error: None,
            filter: String::new(),
            selected: String::new(),
            follow: true,
            reset_plot: false,
        }
    }

    /// File access happens once outside egui's repeatable layout and world locks.
    pub fn poll(&mut self) {
        if Instant::now() < self.next_poll {
            return;
        }
        self.next_poll = Instant::now() + Duration::from_millis(200);
        let Some(reader) = &mut self.reader else {
            return;
        };
        match reader.poll() {
            Ok(records) => {
                self.error = None;
                if !records.is_empty() {
                    // Catch up a bounded batch each frame; wait only when caught up.
                    self.next_poll = Instant::now();
                    self.reset_plot |= self.follow;
                }
                for record in records {
                    self.history.ingest(record);
                }
                if self.selected.is_empty() {
                    self.selected = self.history.names().next().unwrap_or_default().to_owned();
                }
            }
            Err(error) => self.error = Some(format!("{error:#}")),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let Some(path) = &self.path else {
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
            ui.label(format!("{} loaded windows", self.history.total_windows));
            if let Some(tick) = self.history.last_tick() {
                ui.label(format!("Latest recorded tick {tick}"));
            }
            if self.reader.as_ref().is_some_and(Reader::has_pending_record) {
                ui.weak("Waiting for the remainder of a record…");
            }
            if self.history.total_windows > self.history.len() as u64 {
                ui.weak(format!(
                    "Showing the latest {WINDOW_LIMIT} windows; the file retains all history."
                ));
            }
        });
        if self.history.len() == 0 {
            ui.add_space(12.0);
            ui.label("Waiting for recorded metric windows…");
            ui.weak(
                "Enable collection with configs/metrics.toml. New records appear automatically.",
            );
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Find metric");
            ui.text_edit_singleline(&mut self.filter);
            if ui.checkbox(&mut self.follow, "Follow data").changed() {
                self.reset_plot = true;
            }
            if ui.button("Fit").clicked() {
                self.reset_plot = true;
            }
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
                for name in self
                    .history
                    .names()
                    .filter(|name| name.to_lowercase().contains(&filter))
                {
                    if ui
                        .selectable_value(&mut self.selected, name.to_owned(), name)
                        .changed()
                    {
                        self.reset_plot = true;
                    }
                }
            });
        if self.selected.is_empty() {
            ui.label("No scalar metrics are present in the recorded summaries.");
            return;
        }
        ui.weak(
            "Drag to pan · scroll to zoom · double-click to fit · diamonds mark partial windows",
        );
        let segments = self.history.segments(&self.selected);
        if segments.is_empty() {
            ui.label("This metric has no values in the retained windows.");
            return;
        }
        let mut plot = Plot::new(("metrics-plot", &self.selected))
            .legend(Legend::default())
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
        if std::mem::take(&mut self.reset_plot) {
            plot = plot.reset();
        }
        let response = plot.show(ui, |plot_ui| {
            for segment in &segments {
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
        // A deliberate viewport change should persist while new windows arrive.
        if response.response.dragged()
            || (response.response.hovered()
                && ui.input(|input| {
                    input.smooth_scroll_delta != egui::Vec2::ZERO || input.zoom_delta() != 1.0
                }))
        {
            self.follow = false;
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
