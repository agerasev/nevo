use nevo::{
    environment::ecosystem::Ecosystem,
    run::{Command, Control, Run},
};

/// Exercise the command channel while checking the snapshots actually presented.
#[derive(Clone)]
pub struct SmokeCheck {
    reference: Run,
    previous_tick: u64,
}

impl SmokeCheck {
    pub fn new(reference: Run) -> Self {
        let previous_tick = reference.world().read().number();
        Self {
            reference,
            previous_tick,
        }
    }

    pub fn command(&self, frame: u32, control: &Control) -> wgame::Result<()> {
        let command = match frame {
            0 | 7 => Some(Command::Pause),
            1 | 3 | 9 | 10 => Some(Command::Step),
            4 => Some(Command::Resume),
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
        snapshot: &Ecosystem,
        control: &Control,
    ) -> wgame::Result<()> {
        let tick = snapshot.number();
        let expected = match frame {
            0 => Some(self.previous_tick),
            1 | 3 | 9 | 10 => Some(self.previous_tick + 1),
            2 | 8 | 11 => Some(self.previous_tick),
            _ => None,
        };
        let paused = !matches!(frame, 4..=6);
        if expected.is_some_and(|expected| tick != expected) || control.status().paused != paused {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: expected tick {expected:?}, paused={paused}; got tick {tick}, paused={}",
                control.status().paused
            )));
        }
        while self.reference.world().read().number() < tick {
            self.reference.tick();
        }
        if *self.reference.world().read() != *snapshot {
            return Err(wgame::Error::msg(format!(
                "frame {frame}: snapshot differs from the seeded reference at tick {tick}"
            )));
        }
        self.previous_tick = tick;
        if frame == 11 {
            println!("Verified 12 displayed snapshots and pause/step commands; final tick {tick}.");
        }
        Ok(())
    }
}
