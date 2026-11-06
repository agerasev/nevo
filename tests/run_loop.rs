use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use nevo::{
    run::{Command, Control, LoopOptions, Run, StopReason},
    universe::{Simulation, Universe},
};

// Deliberately not Clone: the runner must work without copying its model.
struct Counter(u64);
impl Simulation for Counter {
    type Config = ();
    type Update = u64;
    type Report = u64;
    fn compute_update(&self, _: u64) -> anyhow::Result<(u64, u64)> {
        Ok((self.0 + 1, self.0 + 1))
    }
    fn apply_update(&mut self, value: u64) {
        self.0 = value;
    }
}

const TIMEOUT: Duration = Duration::from_secs(3);

fn run() -> Run<Counter> {
    Run::from_universe(Universe::from_world(Counter(0)).unwrap())
}

fn start(paused: bool, interval: Duration) -> (Control, Receiver<u64>, Receiver<StopReason>) {
    let (control, commands) = Control::channel(paused);
    let (observed, ticks) = mpsc::channel();
    let (done, stopped) = mpsc::channel();
    thread::spawn(move || {
        let reason = run()
            .execute(
                LoopOptions {
                    ticks: None,
                    interval,
                },
                commands,
                |_, tick| {
                    observed.send(*tick).unwrap();
                    false
                },
            )
            .unwrap();
        done.send(reason).unwrap();
    });
    (control, ticks, stopped)
}

fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !ready() {
        assert!(Instant::now() < deadline, "worker did not apply command");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_single_step_leaves_the_loop_paused() {
    let (control, ticks, stopped) = start(false, Duration::from_secs(10));
    control.send(Command::Step).unwrap();
    assert_eq!(ticks.recv_timeout(TIMEOUT).unwrap(), 1);
    assert!(control.status().paused);
    assert!(ticks.recv_timeout(Duration::from_millis(30)).is_err());
    control.send(Command::Stop).unwrap();
    assert_eq!(
        stopped.recv_timeout(TIMEOUT).unwrap(),
        StopReason::Requested
    );
}

#[test]
fn pause_and_resume_gate_ticks() {
    let (control, ticks, stopped) = start(true, Duration::from_millis(5));
    control.send(Command::Resume).unwrap();
    assert_eq!(ticks.recv_timeout(TIMEOUT).unwrap(), 1);
    control.send(Command::Pause).unwrap();
    wait_until(|| control.status().paused);
    while ticks.try_recv().is_ok() {}
    assert!(ticks.recv_timeout(Duration::from_millis(30)).is_err());
    control.send(Command::Stop).unwrap();
    stopped.recv_timeout(TIMEOUT).unwrap();
}

#[test]
fn changing_speed_interrupts_a_pacing_wait() {
    let slow = Duration::from_secs(10);
    let (control, ticks, stopped) = start(false, slow);
    wait_until(|| control.status().interval == slow);
    control.send(Command::SetInterval(Duration::ZERO)).unwrap();
    assert_eq!(ticks.recv_timeout(TIMEOUT).unwrap(), 1);
    control.send(Command::Stop).unwrap();
    stopped.recv_timeout(TIMEOUT).unwrap();
}

#[test]
fn changing_speed_does_not_unpause() {
    let (control, ticks, stopped) = start(true, Duration::ZERO);
    let interval = Duration::from_millis(5);
    control.send(Command::SetInterval(interval)).unwrap();
    wait_until(|| control.status().interval == interval);
    assert!(control.status().paused);
    assert!(ticks.recv_timeout(Duration::from_millis(30)).is_err());
    control.send(Command::Stop).unwrap();
    stopped.recv_timeout(TIMEOUT).unwrap();
}

#[test]
fn tick_limits_apply_to_each_invocation() {
    let mut run = run();
    for expected in [2, 4] {
        let (_control, commands) = Control::channel(false);
        let stop = run
            .execute(
                LoopOptions {
                    ticks: Some(2),
                    ..Default::default()
                },
                commands,
                |_, _| false,
            )
            .unwrap();
        assert_eq!(stop, StopReason::TickLimit);
        assert_eq!(run.universe().read().world().0, expected);
    }
}

#[test]
fn a_stop_condition_observes_the_published_tick() {
    let mut run = run();
    let (_control, commands) = Control::channel(false);
    let stop = run
        .execute(LoopOptions::default(), commands, |run, tick| {
            assert_eq!(run.universe().read().world().0, *tick);
            *tick == 2
        })
        .unwrap();
    assert_eq!(stop, StopReason::Condition);
    assert_eq!(run.universe().read().step(), 2);
}

#[test]
fn zero_tick_limit_finishes_while_paused() {
    let mut run = run();
    let (_control, commands) = Control::channel(true);
    let stop = run
        .execute(
            LoopOptions {
                ticks: Some(0),
                ..Default::default()
            },
            commands,
            |_, _| panic!("no tick"),
        )
        .unwrap();
    assert_eq!(stop, StopReason::TickLimit);
    assert_eq!(run.universe().read().step(), 0);
}

#[test]
fn dropping_last_control_wakes_a_paused_loop() {
    let (control, _, stopped) = start(true, Duration::from_secs(10));
    wait_until(|| control.status().interval == Duration::from_secs(10));
    drop(control);
    assert_eq!(
        stopped.recv_timeout(TIMEOUT).unwrap(),
        StopReason::Disconnected
    );
}
