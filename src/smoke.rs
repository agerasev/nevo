use std::time::Duration;

use nevo::{foraging::Scenario, run::Run};

use crate::playback::{Command, Playback};

/// Fixed input and elapsed times for the bounded window smoke check.
pub struct SmokeCheck {
    headless: Run,
    initially_paused: bool,
}

impl SmokeCheck {
    pub fn new(scenario: Scenario, seed: u64, paused: bool) -> Self {
        Self {
            headless: Run::new(scenario, seed),
            initially_paused: paused,
        }
    }

    /// (elapsed milliseconds, input, expected displayed tick, expected pause state)
    fn stage(&self, frame: u32) -> (u64, Option<Command>, u64, bool) {
        use Command::{Step, TogglePause};
        match frame {
            0 => (0, None, 0, self.initially_paused),
            1 => (0, self.initially_paused.then_some(TogglePause), 0, false),
            2 => (120, None, 3, false),
            3 => (80, Some(TogglePause), 3, true),
            4 => (60_000, None, 3, true),
            5 => (60_000, Some(Step), 4, true),
            6 => (0, Some(Step), 5, true),
            7 => (60_000, Some(TogglePause), 5, false),
            8 => (40, None, 6, false),
            9 => (80, None, 8, false),
            10 => (40, Some(Step), 9, true),
            11 => (60_000, None, 9, true),
            _ => unreachable!("smoke check has twelve frames"),
        }
    }

    pub fn frame(&self, frame: u32) -> (Duration, Vec<Command>) {
        let (millis, command, _, _) = self.stage(frame);
        (Duration::from_millis(millis), command.into_iter().collect())
    }

    pub fn verify(&mut self, frame: u32, playback: &Playback) -> wgame::Result<()> {
        let (_, _, tick, paused) = self.stage(frame);
        if playback.run.world.number() != tick || playback.paused() != paused {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: expected tick {tick}, paused={paused}; got tick {}, paused={}",
                playback.run.world.number(),
                playback.paused()
            )));
        }
        while self.headless.world.number() < tick {
            self.headless.tick();
        }
        if playback.run != self.headless {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: viewer differs from headless at tick {tick}"
            )));
        }
        if frame == 11 {
            println!(
                "Verified 12 presented frames against headless; final tick {tick}, paused={paused}."
            );
        }
        Ok(())
    }
}
