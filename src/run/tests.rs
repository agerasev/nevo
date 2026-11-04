use super::*;
use crate::actor::HandleAllocator;
use anyhow::{Result, ensure};
use rand::{Rng, SeedableRng};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum Observation {
    Input(u64),
    Feedback,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct World {
    value: u64,
    fail: bool,
}

impl Environment for World {
    type Observation = Observation;
    type Action = u64;
    type Report = u64;
    type Request = ();
    type Intervention = ();

    fn observe(&self) -> Observation {
        Observation::Input(self.value)
    }

    fn advance(
        &mut self,
        _: u64,
        action: u64,
        handles: &mut HandleAllocator,
        mut intervene: impl FnMut(()) -> Result<()>,
    ) -> Result<(u64, Observation)> {
        self.value = action;
        handles.allocate()?;
        ensure!(!self.fail, "environment failed");
        intervene(())?;
        Ok((self.value, Observation::Feedback))
    }

    fn validate(&self, _: &HandleAllocator) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Probe {
    pending: Option<u64>,
    memory: u64,
    fail_action: bool,
    fail_feedback: bool,
    #[serde(skip)]
    inspect: Option<Arc<dyn Fn() + Send + Sync>>,
}

type TestContext = Cpu<Xoshiro256PlusPlus>;

impl Actor<TestContext> for Probe {
    type Observation = Observation;
    type Action = u64;

    fn observe(&mut self, _: &mut TestContext, observation: Observation) -> Result<()> {
        match observation {
            Observation::Input(value) => self.pending = Some(value),
            Observation::Feedback => {
                self.memory += 1;
                ensure!(!self.fail_feedback, "feedback failed");
            }
        }
        Ok(())
    }

    fn act(&mut self, context: &mut TestContext) -> Result<u64> {
        if let Some(inspect) = &self.inspect {
            inspect();
        }
        self.memory = context.rng.next_u64() % 100;
        ensure!(!self.fail_action, "actor failed");
        Ok(self.pending.take().unwrap() + 1)
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct TestPolicy {
    calls: u64,
    fail: bool,
}

impl Policy<()> for TestPolicy {
    type Intervention = ();
    fn respond(&mut self, _: ()) -> Result<()> {
        self.calls += 1;
        ensure!(!self.fail, "policy failed");
        Ok(())
    }
}

type TestUniverse = Universe<World, Probe, TestContext, TestPolicy>;
type TestRun = Run<World, Probe, TestContext, TestPolicy>;

fn universe() -> TestUniverse {
    Universe::from_parts(
        0,
        World {
            value: 0,
            fail: false,
        },
        Probe::default(),
        Cpu {
            rng: Xoshiro256PlusPlus::seed_from_u64(42),
        },
        TestPolicy::default(),
        HandleAllocator::default(),
    )
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
    run.universe.0.write().unwrap().actor.inspect = Some(Arc::new(move || {
        assert!(view.0.try_write().is_err(), "preparation holds a read lock");
        let state = view.0.try_read().expect("concurrent readers are allowed");
        assert_eq!(state.step(), 0);
        assert_eq!(state.actor.memory, 0);
        assert_eq!(state.environment.value, 0);
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
                assert_eq!(state.environment.value, *report);
                assert_eq!(state.policy.calls, 1);
                false
            },
        )
        .unwrap();
    assert_eq!(stop, StopReason::TickLimit);
    assert_eq!(observed.load(Ordering::Relaxed), 1);
    // Remove the test instrumentation's back-reference.
    run.universe.0.write().unwrap().actor.inspect = None;
}

#[test]
fn readers_can_hold_back_publication_while_preparation_runs() {
    let mut run = TestRun::from_universe(universe());
    let view = run.universe.clone();
    let (entered, acting) = mpsc::channel();
    let (release, proceed) = mpsc::channel();
    let proceed = Mutex::new(proceed);
    run.universe.0.write().unwrap().actor.inspect = Some(Arc::new(move || {
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
    assert_eq!(old.environment.value, 0);
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
fn failed_preparation_preserves_environment_actor_policy_allocator_and_rng() {
    for phase in 0..4 {
        let mut state = universe();
        match phase {
            0 => state.actor.fail_action = true,
            1 => state.environment.fail = true,
            2 => state.policy.fail = true,
            3 => state.actor.fail_feedback = true,
            _ => unreachable!(),
        }
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
