use std::time::Duration;

use nevo::{
    run::{Command, Control, Run, tick_interval},
    universe::Universe,
};

/// Exercise the command channel while checking the snapshots actually presented.
#[derive(Clone)]
pub struct SmokeCheck {
    reference: Run,
    previous_tick: u64,
}

impl SmokeCheck {
    pub fn new(reference: Run) -> Self {
        let previous_tick = reference.universe().read().step();
        Self {
            reference,
            previous_tick,
        }
    }

    pub fn command(&self, frame: u32, control: &Control) -> wgame::Result<()> {
        let command = match frame {
            0 | 7 => Some(Command::Pause),
            1 | 3 | 9 | 10 => Some(Command::Step),
            2 => Some(Command::SetInterval(tick_interval(60.0).unwrap())),
            4 => Some(Command::Resume),
            5 => Some(Command::SetInterval(Duration::ZERO)),
            _ => None,
        };
        if let Some(command) = command {
            control.request(command).map_err(wgame::Error::msg)?;
        }
        Ok(())
    }

    pub fn verify(
        &mut self,
        frame: u32,
        snapshot: &Universe,
        control: &Control,
    ) -> wgame::Result<()> {
        let tick = snapshot.step();
        let expected = match frame {
            0 => Some(self.previous_tick),
            1 | 3 | 9 | 10 => Some(self.previous_tick + 1),
            2 | 8 | 11 => Some(self.previous_tick),
            _ => None,
        };
        let paused = !matches!(frame, 4..=6);
        let expected_interval = match frame {
            2..=4 => Some(tick_interval(60.0).unwrap()),
            5..=11 => Some(Duration::ZERO),
            _ => None,
        };
        if expected_interval.is_some_and(|interval| control.status().interval != interval) {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: pacing command was not applied"
            )));
        }
        if expected.is_some_and(|expected| tick != expected) || control.status().paused != paused {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: expected tick {expected:?}, paused={paused}; got tick {tick}, paused={}",
                control.status().paused
            )));
        }
        while self.reference.universe().read().step() < tick {
            self.reference
                .tick()
                .map_err(|error| wgame::Error::msg(error.to_string()))?;
        }
        if *self.reference.universe().read() != *snapshot {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: snapshot differs from the seeded reference at tick {tick}"
            )));
        }
        self.previous_tick = tick;
        Ok(())
    }
}
