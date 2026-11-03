use std::time::Duration;

use nevo::{foraging::Scenario, run::Run};

pub const TICK_INTERVAL: Duration = Duration::from_millis(40);
const MAX_TICKS_PER_UPDATE: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    TogglePause,
    Step,
}

pub struct Playback {
    pub run: Run,
    paused: bool,
    accumulated: Duration,
}

impl Playback {
    pub fn new(scenario: Scenario, seed: u64, paused: bool) -> Self {
        Self {
            run: Run::new(scenario, seed),
            paused,
            accumulated: Duration::ZERO,
        }
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Delay until the next automatic tick. Paused playback can wait for input.
    pub fn wait_duration(&self) -> Option<Duration> {
        (!self.paused).then(|| TICK_INTERVAL - self.accumulated)
    }

    pub fn advance(&mut self, elapsed: Duration, commands: &[Command]) {
        if !commands.is_empty() {
            // Each control starts a fresh clock interval so resuming can't spend
            // paused time and a single step can't trigger any automatic ticks.
            self.accumulated = Duration::ZERO;
            for command in commands {
                match command {
                    Command::TogglePause => self.paused = !self.paused,
                    Command::Step => {
                        self.paused = true;
                        self.run.tick();
                    }
                }
            }
            return;
        }
        if self.paused {
            return;
        }
        // Bound catch-up work after a delayed redraw. Every tick still advances
        // the simulation by the same amount as a tick of the headless run.
        self.accumulated += elapsed.min(TICK_INTERVAL * MAX_TICKS_PER_UPDATE);
        while self.accumulated >= TICK_INTERVAL {
            self.run.tick();
            self.accumulated -= TICK_INTERVAL;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nevo::foraging::Event;

    #[test]
    fn pause_step_and_resume_preserve_the_live_run() {
        let mut player = Playback::new(Scenario::Debug, 14, false);
        let mut headless = Run::new(Scenario::Debug, 14);

        player.advance(Duration::from_millis(55), &[]);
        let first = headless.tick();
        assert!(
            first
                .events
                .iter()
                .any(|event| matches!(event, Event::Birth { .. }))
        );
        assert_eq!(player.run, headless);
        assert_eq!(player.wait_duration(), Some(Duration::from_millis(25)));

        player.advance(Duration::from_secs(2), &[Command::TogglePause]);
        assert!(player.paused());
        assert_eq!(player.wait_duration(), None);
        player.advance(Duration::MAX, &[]);
        assert_eq!(player.run, headless);
        player.advance(Duration::from_secs(1), &[Command::Step]);
        headless.tick();
        assert!(player.paused());
        assert_eq!(player.run, headless);

        player.advance(Duration::MAX, &[Command::TogglePause]);
        assert_eq!(player.run, headless);
        assert_eq!(player.wait_duration(), Some(TICK_INTERVAL));
        player.advance(Duration::from_millis(39), &[]);
        assert_eq!(player.run, headless);
        player.advance(Duration::from_millis(1), &[]);
        headless.tick();
        assert_eq!(player.run, headless);

        player.advance(Duration::MAX, &[Command::Step]);
        headless.tick();
        assert!(player.paused());
        assert_eq!(player.run, headless);
    }

    #[test]
    fn delayed_redraws_match_headless_through_baseline_turnover() {
        let mut player = Playback::new(Scenario::Baseline, 42, false);
        let mut headless = Run::new(Scenario::Baseline, 42);
        let mut saw_immigration = false;
        for _ in 0..32 {
            player.advance(Duration::MAX, &[]);
            for _ in 0..MAX_TICKS_PER_UPDATE {
                let report = headless.tick();
                saw_immigration |= report
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Immigrant { .. }));
            }
            assert_eq!(player.run, headless);
            assert_eq!(player.wait_duration(), Some(TICK_INTERVAL));
        }
        assert!(saw_immigration);
    }
}
