use super::*;
use anyhow::{Result, ensure};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::sync::atomic::{AtomicUsize, Ordering};

/// An unrelated experiment demonstrates that the runner requires no animal,
/// perception, lifecycle, or population contracts.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Probe {
    value: u64,
    memory: u64,
    rng: Xoshiro256PlusPlus,
    births: u64,
    interventions: u64,
    fail_phase: u8,
    #[serde(skip)]
    inspect: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Simulation for Probe {
    type Report = u64;

    fn advance(&mut self, _: u64) -> Result<u64> {
        if let Some(inspect) = &self.inspect {
            inspect();
        }
        self.memory = self.rng.next_u64();
        ensure!(self.fail_phase != 1, "mind failed");
        self.value += 1;
        self.births += 1;
        ensure!(self.fail_phase != 2, "physical update failed");
        self.interventions += 1;
        ensure!(self.fail_phase != 3, "intervention failed");
        Ok(self.value)
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
        inspect: None,
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
    // Remove the test instrumentation's back-reference.
    run.universe.0.write().unwrap().world.inspect = None;
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
