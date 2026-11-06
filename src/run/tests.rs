use super::*;
use anyhow::{Result, ensure};
use std::sync::atomic::{AtomicUsize, Ordering};

/// An unrelated experiment demonstrates that the runner requires no animal,
/// perception, lifecycle, or population contracts.
// Deliberately not Clone: stepping must prepare changes without copying the model.
struct Probe {
    value: u64,
    fail: bool,
    increment: u64,
    configurations: u32,
    inspect: Option<Arc<dyn Fn() + Send + Sync>>,
    inspect_apply: Option<Arc<dyn Fn() + Send + Sync>>,
}

// Deliberately neither Clone nor serializable: settings are applied once.
struct ProbeConfig {
    increment: u64,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self { increment: 1 }
    }
}

impl Simulation for Probe {
    type Config = ProbeConfig;
    type Update = u64;
    type Report = u64;

    fn configure(&mut self, config: &ProbeConfig) -> Result<()> {
        ensure!(config.increment > 0, "increment must be positive");
        self.increment = config.increment;
        self.configurations += 1;
        Ok(())
    }

    fn compute_update(&self, _: u64) -> Result<(u64, u64)> {
        if let Some(inspect) = &self.inspect {
            inspect();
        }
        ensure!(!self.fail, "preparation failed");
        let value = self.value + self.increment;
        Ok((value, value))
    }

    fn apply_update(&mut self, update: u64) {
        if let Some(inspect) = &self.inspect_apply {
            inspect();
        }
        self.value = update;
    }
}

type TestUniverse = Universe<Probe>;
type TestRun = Run<Probe>;

fn universe() -> TestUniverse {
    Universe::from_world(Probe {
        value: 0,
        fail: false,
        increment: 1,
        configurations: 0,
        inspect: None,
        inspect_apply: None,
    })
    .unwrap()
}

#[test]
fn preparation_holds_read_lock_and_callback_runs_after_atomic_publication() {
    let mut run = TestRun::from_universe(universe());
    let observed = Arc::new(AtomicUsize::new(0));
    let view = run.universe.clone();
    let count = observed.clone();
    run.universe.0.write().unwrap().world.inspect = Some(Arc::new(move || {
        assert!(view.0.try_write().is_err(), "preparation holds a read lock");
        let state = view.0.try_read().expect("concurrent readers are allowed");
        assert_eq!(state.step(), 0);
        assert_eq!(state.world.value, 0);
        count.fetch_add(1, Ordering::Relaxed);
    }));
    let applied = Arc::new(AtomicUsize::new(0));
    let count = applied.clone();
    let view = run.universe.clone();
    run.universe.0.write().unwrap().world.inspect_apply = Some(Arc::new(move || {
        assert!(view.0.try_read().is_err(), "application excludes readers");
        assert!(
            view.0.try_write().is_err(),
            "application holds the write lock"
        );
        count.fetch_add(1, Ordering::Relaxed);
    }));
    let (_control, commands) = Control::channel(false);
    let stop = run
        .execute(
            LoopOptions {
                ticks: Some(1),
                interval: Duration::ZERO,
            },
            commands,
            |run, report| {
                let state = run
                    .universe
                    .0
                    .try_write()
                    .expect("callback holds no universe lock");
                assert_eq!(state.step(), 1);
                assert_eq!(state.world.value, *report);
                false
            },
        )
        .unwrap();
    assert_eq!(stop, StopReason::TickLimit);
    assert_eq!(observed.load(Ordering::Relaxed), 1);
    assert_eq!(applied.load(Ordering::Relaxed), 1);
    // Remove the test instrumentation's back-reference.
    run.universe.0.write().unwrap().world.inspect = None;
    run.universe.0.write().unwrap().world.inspect_apply = None;
}

#[test]
fn exhausted_step_counter_fails_before_model_preparation() {
    let mut state = universe();
    state.step = u64::MAX - 1;
    state.world.inspect = Some(Arc::new(|| panic!("must check counter before preparation")));
    assert!(state.advance().is_err());
    assert_eq!(state.step, u64::MAX - 1);
}

#[test]
fn restore_configures_once_and_steps_use_stored_component_settings() {
    let mut run = universe().restore(ProbeConfig { increment: 7 }).unwrap();
    assert_eq!(run.tick().unwrap(), 7);
    assert_eq!(run.tick().unwrap(), 14);
    assert_eq!(run.universe().read().world().value, 14);
    assert_eq!(run.universe().read().world().increment, 7);
    assert_eq!(run.universe().read().world().configurations, 1);
}

#[test]
fn failed_preparation_preserves_state_and_does_not_call_observers() {
    let mut state = universe();
    state.world.fail = true;
    let mut run = TestRun::from_universe(state);
    let (control, commands) = Control::channel(false);
    assert!(
        run.execute(LoopOptions::default(), commands, |_, _| panic!(
            "failed tick"
        ))
        .is_err()
    );
    assert_eq!(control.status().stop, Some(StopReason::Failed));
    assert_eq!(control.status().steps, 0);
    assert_eq!(run.universe().read().step(), 0);
    assert_eq!(run.universe().read().world().value, 0);
}
