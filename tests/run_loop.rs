use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use nevo::{
    AgentId, AgentResponse, Environment, Population,
    foraging::Scenario,
    run::{Command, Control, LoopOptions, Run, StopReason},
};

const TIMEOUT: Duration = Duration::from_secs(3);

fn request(control: &Control, command: Command) {
    let control = control.clone();
    let (done, result) = mpsc::channel();
    thread::spawn(move || done.send(control.request(command)).unwrap());
    result
        .recv_timeout(TIMEOUT)
        .expect("control acknowledgement")
        .unwrap();
}

#[test]
fn pause_and_step_commands_control_the_same_worker() {
    let mut run = Run::new(Scenario::Baseline, 42);
    let mut expected = run.clone();
    let world = run.world().clone();
    let (control, commands) = Control::channel(true);
    let (observed, ticks) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let stop = run.execute(
            LoopOptions {
                interval: Duration::from_millis(10),
                ..LoopOptions::default()
            },
            commands,
            |_, report| {
                observed.send(report.tick).unwrap();
                false
            },
        );
        done.send((run, stop)).unwrap();
    });

    request(&control, Command::Pause);
    assert_eq!(world.read().number(), 0);
    for tick in 1..=3 {
        request(&control, Command::Step);
        assert_eq!(ticks.recv_timeout(TIMEOUT).unwrap(), tick);
        expected.tick();
        assert_eq!(world.read().number(), tick);
        assert_eq!(*world.read(), *expected.world().read());
        assert!(control.status().paused);
    }
    thread::sleep(Duration::from_millis(80));
    assert_eq!(
        world.read().number(),
        3,
        "paused time must not advance ticks"
    );
    request(&control, Command::Resume);
    assert!(ticks.recv_timeout(TIMEOUT).unwrap() > 3);
    request(&control, Command::Pause);
    let paused_at = world.read().number();
    request(&control, Command::Pause);
    assert_eq!(world.read().number(), paused_at);
    request(&control, Command::Step);
    assert_eq!(world.read().number(), paused_at + 1);
    request(&control, Command::Stop);
    let (actual, stop) = finished.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(stop, StopReason::Requested);
    while expected.world().read().number() < actual.world().read().number() {
        expected.tick();
    }
    assert_eq!(actual, expected);
}

#[test]
fn zero_tick_limit_works_while_paused() {
    let mut run = Run::new(Scenario::Debug, 1);
    let original = run.clone();
    let (_control, commands) = Control::channel(true);
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let reason = run.execute(
            LoopOptions {
                ticks: Some(0),
                ..LoopOptions::default()
            },
            commands,
            |_, _| false,
        );
        done.send((run, reason)).unwrap();
    });
    let (run, reason) = finished.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(reason, StopReason::TickLimit);
    assert_eq!(run, original);
}

#[test]
fn dropping_last_control_wakes_a_paused_loop() {
    let mut run = Run::new(Scenario::Debug, 1);
    let original = run.clone();
    let (control, commands) = Control::channel(true);
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let reason = run.execute(LoopOptions::default(), commands, |_, _| false);
        done.send((run, reason)).unwrap();
    });
    request(&control, Command::Pause);
    drop(control);
    let (run, reason) = finished.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(reason, StopReason::Disconnected);
    assert_eq!(run, original);
}

#[derive(Debug)]
struct World(u64);

struct SlowAgent {
    entered: Sender<()>,
    release: Receiver<()>,
}

impl Population<u64, u64, ()> for SlowAgent {
    fn spawn(&mut self, _: AgentId, _: ()) {}
    fn despawn(&mut self, _: AgentId) {}
    fn act(&mut self, _: AgentId, observation: &u64) -> u64 {
        self.entered.send(()).unwrap();
        self.release
            .recv_timeout(TIMEOUT)
            .expect("release slow agent");
        observation + 1
    }
}

impl Environment<SlowAgent> for World {
    type Observation = u64;
    type Action = u64;
    type Origin = ();
    type Report = u64;

    fn observe(&self) -> Vec<(AgentId, u64)> {
        vec![(AgentId(1), self.0)]
    }

    fn update(&mut self, _: &mut SlowAgent, responses: Vec<AgentResponse<u64, u64>>) -> u64 {
        assert_eq!(responses[0].observation, self.0);
        self.0 = responses[0].action;
        self.0
    }
}

#[test]
fn readers_can_inspect_during_agent_work_and_hold_back_publication() {
    let (entered, acting) = mpsc::channel();
    let (release, proceed) = mpsc::channel();
    let mut run = Run::from_parts(
        World(0),
        SlowAgent {
            entered,
            release: proceed,
        },
    );
    let view = run.world().clone();
    let (_control, commands) = Control::channel(false);
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let reason = run.execute(
            LoopOptions {
                ticks: Some(1),
                interval: Duration::ZERO,
            },
            commands,
            |_, _| false,
        );
        done.send(reason).unwrap();
    });
    acting
        .recv_timeout(TIMEOUT)
        .expect("agent entered unlocked phase");
    // Reading remains possible while the agent deliberately stalls.
    let world = view.read();
    assert_eq!(world.0, 0);
    release.send(()).unwrap();
    assert!(matches!(
        finished.recv_timeout(Duration::from_millis(30)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(world.0, 0);
    drop(world);
    assert_eq!(
        finished.recv_timeout(TIMEOUT).unwrap(),
        StopReason::TickLimit
    );
    assert_eq!(view.read().0, 1);
}
