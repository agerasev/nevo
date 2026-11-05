use super::*;
use anyhow::{Result, ensure};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::sync::atomic::{AtomicUsize, Ordering};

/// An unrelated experiment demonstrates that the runner requires no animal,
/// perception, lifecycle, or population contracts.
// Deliberately not Clone: stepping must prepare changes without copying the model.
#[derive(serde::Serialize, serde::Deserialize)]
struct Probe {
    value: u64,
    memory: u64,
    rng: Xoshiro256PlusPlus,
    births: u64,
    interventions: u64,
    fail_phase: u8,
    #[serde(skip)]
    increment: u64,
    #[serde(skip)]
    configurations: u32,
    #[serde(skip)]
    inspect: Option<Arc<dyn Fn() + Send + Sync>>,
    #[serde(skip)]
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

struct ProbeUpdate {
    value: u64,
    memory: u64,
    rng: Xoshiro256PlusPlus,
    births: u64,
    interventions: u64,
}

impl Simulation for Probe {
    type Config = ProbeConfig;
    type Update = ProbeUpdate;
    type Report = u64;

    fn configure(&mut self, config: &ProbeConfig) -> Result<()> {
        ensure!(config.increment > 0, "increment must be positive");
        self.increment = config.increment;
        self.configurations += 1;
        Ok(())
    }

    fn compute_update(&self, _: u64) -> Result<(u64, ProbeUpdate)> {
        if let Some(inspect) = &self.inspect {
            inspect();
        }
        let mut rng = self.rng.clone();
        let memory = rng.next_u64();
        ensure!(self.fail_phase != 1, "mind failed");
        let value = self.value + self.increment;
        let births = self.births + 1;
        ensure!(self.fail_phase != 2, "physical update failed");
        let interventions = self.interventions + 1;
        ensure!(self.fail_phase != 3, "intervention failed");
        ensure!(self.fail_phase != 4, "invalid prepared state");
        Ok((
            value,
            ProbeUpdate {
                value,
                memory,
                rng,
                births,
                interventions,
            },
        ))
    }

    fn apply_update(&mut self, update: ProbeUpdate) {
        if let Some(inspect) = &self.inspect_apply {
            inspect();
        }
        self.value = update.value;
        self.memory = update.memory;
        self.rng = update.rng;
        self.births = update.births;
        self.interventions = update.interventions;
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.fail_phase != 4 || self.value == 0,
            "invalid prepared state"
        );
        Ok(())
    }
}

type TestUniverse = Universe<Probe>;
type TestRun = Run<Probe>;

fn universe() -> TestUniverse {
    Universe::from_world(Probe {
        value: 0,
        memory: 0,
        rng: Xoshiro256PlusPlus::seed_from_u64(42),
        births: 0,
        interventions: 0,
        fail_phase: 0,
        increment: 1,
        configurations: 0,
        inspect: None,
        inspect_apply: None,
    })
    .unwrap()
}

fn encoded(state: &TestUniverse) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(state, &mut bytes).unwrap();
    bytes
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
        assert_eq!(state.world.memory, 0);
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
                assert_eq!(state.world.interventions, 1);
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
fn discarding_an_update_preserves_state_and_randomness() {
    let mut state = universe();
    let before = encoded(&state);
    let _ = state.compute_update().unwrap();
    assert_eq!(encoded(&state), before);

    let mut expected_rng = state.world.rng.clone();
    let expected_memory = expected_rng.next_u64();
    assert_eq!(state.advance().unwrap(), 1);
    assert_eq!(state.step(), 1);
    assert_eq!(state.world.memory, expected_memory);
    assert_eq!(state.world.rng, expected_rng);
}

#[test]
fn exhausted_step_counter_fails_before_model_preparation() {
    let mut state = universe();
    state.step = u64::MAX - 1;
    state.world.inspect = Some(Arc::new(|| panic!("must check counter before preparation")));
    let before = encoded(&state);
    assert!(state.advance().is_err());
    assert_eq!(encoded(&state), before);
}

#[test]
fn readers_can_hold_back_publication_while_preparation_runs() {
    let mut run = TestRun::from_universe(universe());
    let view = run.universe.clone();
    let (entered, acting) = mpsc::channel();
    let (release, proceed) = mpsc::channel();
    let proceed = Mutex::new(proceed);
    run.universe.0.write().unwrap().world.inspect = Some(Arc::new(move || {
        entered.send(()).unwrap();
        proceed
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
    }));
    let (done, completed) = mpsc::channel();
    std::thread::spawn(move || {
        done.send(run.tick()).unwrap();
    });
    acting.recv_timeout(Duration::from_secs(3)).unwrap();
    let old = view.read();
    assert_eq!(old.step(), 0);
    release.send(()).unwrap();
    assert!(matches!(
        completed.recv_timeout(Duration::from_millis(30)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(old.world.value, 0);
    drop(old);
    assert_eq!(
        completed
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap(),
        1
    );
    assert_eq!(view.read().step(), 1);
}

#[test]
fn failed_preparation_preserves_all_experiment_state() {
    for phase in 1..=4 {
        let mut state = universe();
        state.world.fail_phase = phase;
        let before = encoded(&state);
        let mut run = TestRun::from_universe(state);
        let (control, commands) = Control::channel(false);
        assert!(
            run.execute(
                LoopOptions {
                    ticks: Some(1),
                    interval: Duration::ZERO
                },
                commands,
                |_, _| panic!("failed tick cannot be observed")
            )
            .is_err()
        );
        assert_eq!(control.status().stop, Some(StopReason::Failed));
        assert_eq!(control.status().steps, 0);
        assert_eq!(
            encoded(&run.universe.read()),
            before,
            "failure phase {phase}"
        );
    }
}

#[test]
fn restore_configures_once_and_steps_use_stored_component_settings() {
    let mut run = universe().restore(ProbeConfig { increment: 7 }).unwrap();
    assert_eq!(run.tick().unwrap(), 7);
    assert_eq!(run.tick().unwrap(), 14);
    assert_eq!(run.universe().read().world().value, 14);
    assert_eq!(run.universe().read().world().increment, 7);
    assert_eq!(run.universe().read().world().configurations, 1);
    let bytes = encoded(&run.universe().read());
    let decoded: TestUniverse = ciborium::from_reader(bytes.as_slice()).unwrap();
    assert_eq!(decoded.world.increment, 0);
    let mut resumed = decoded.restore(ProbeConfig { increment: 3 }).unwrap();
    assert_eq!(resumed.universe().read().world().configurations, 1);
    assert_eq!(resumed.tick().unwrap(), 17);
    assert_eq!(resumed.universe().read().world().configurations, 1);
    assert!(universe().restore(ProbeConfig { increment: 0 }).is_err());
}

#[test]
fn cloning_a_run_preserves_its_metrics_window_and_accumulation() {
    use crate::metrics::Metrics;

    let mut run = Run::new(crate::test_support::ecosystem("debug"), 1).unwrap();
    for _ in 0..3 {
        run.tick().unwrap();
    }
    let cloned = run.clone();
    let original = run.universe().read();
    let copy = cloned.universe().read();
    assert_eq!(
        original.world().metrics_window(),
        copy.world().metrics_window()
    );
    assert_eq!(
        serde_json::to_value(original.metrics()).unwrap(),
        serde_json::to_value(copy.metrics()).unwrap()
    );
    assert_eq!(copy.world().metrics_window().unwrap().samples, 3);
}
