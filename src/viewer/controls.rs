use std::time::Duration;

use wgame_egui::egui;

use crate::run::{Command, tick_interval};

/// Remember the paced rate while full speed is selected.
pub(super) struct Controls {
    tick_hz: f64,
    paced_hz: f64,
    observed_interval: Duration,
    full_speed_clicked: bool,
}

impl Controls {
    pub fn new(interval: Duration) -> Self {
        let tick_hz = if interval.is_zero() {
            0.0
        } else {
            1.0 / interval.as_secs_f64()
        };
        Self {
            tick_hz,
            paced_hz: if tick_hz == 0.0 { 60.0 } else { tick_hz },
            observed_interval: interval,
            full_speed_clicked: false,
        }
    }

    pub fn begin_frame(&mut self, interval: Duration) {
        self.full_speed_clicked = false;
        // Do not overwrite an edit with the old status while its command is queued.
        if interval != self.observed_interval {
            self.observed_interval = interval;
            if tick_interval(self.tick_hz).ok() != Some(interval) {
                self.tick_hz = if interval.is_zero() {
                    0.0
                } else {
                    1.0 / interval.as_secs_f64()
                };
                if self.tick_hz > 0.0 {
                    self.paced_hz = self.tick_hz;
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, paused: bool, command: impl Fn(Command)) {
        if ui
            .button(if paused { "Resume" } else { "Pause" })
            .on_hover_text("Space when the arena has focus")
            .clicked()
        {
            command(Command::TogglePause);
        }
        if ui
            .button("Single step")
            .on_hover_text("Pause and advance one tick (Right Arrow in the arena)")
            .clicked()
        {
            command(Command::Step);
        }
        ui.separator();
        ui.label("Tick Hz");
        let previous = self.tick_hz;
        ui.add(
            egui::DragValue::new(&mut self.tick_hz)
                .speed(1.0)
                .range(0.0..=1_000_000_000.0)
                .clamp_existing_to_range(false)
                .max_decimals(3)
                .update_while_editing(false)
                .custom_parser(|text| {
                    let hz = text.parse().ok()?;
                    tick_interval(hz).ok()?;
                    Some(hz)
                }),
        )
        .on_hover_text("Ticks per second; 0 is full speed. Click to type, Enter to apply.");
        if self.tick_hz > 0.0 {
            self.paced_hz = self.tick_hz;
        }
        if ui
            .selectable_label(self.tick_hz == 0.0, "Full speed")
            .on_hover_text("Toggle unlimited speed; remembers the last positive tick rate")
            .clicked()
            && !self.full_speed_clicked
        {
            // A layout retry must not toggle the remembered speed a second time.
            self.full_speed_clicked = true;
            self.tick_hz = if self.tick_hz == 0.0 {
                self.paced_hz
            } else {
                0.0
            };
        }
        if self.tick_hz != previous {
            if let Ok(interval) = tick_interval(self.tick_hz) {
                command(Command::SetInterval(interval));
            } else {
                self.tick_hz = previous;
            }
        }
    }
}
