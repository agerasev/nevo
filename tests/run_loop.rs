#[allow(dead_code)]
mod support;

use std::{
    sync::mpsc::{self},
    thread,
    time::Duration,
};

use nevo::run::{Command, Control, LoopOptions, Run, StopReason};

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
    let mut run = Run::new(support::ecosystem("baseline"), 42).unwrap();
    let mut expected = run.clone();
    let world = run.universe().clone();
    let (control, commands) = Control::channel(true);
    let (observed, ticks) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let stop = run
            .execute(
                LoopOptions {
                    interval: Duration::from_millis(10),
                    ..LoopOptions::default()
                },
                commands,
                |_, report| {
                    observed.send(report.tick).unwrap();
                    false
                },
            )
            .unwrap();
        done.send((run, stop)).unwrap();
    });

    request(&control, Command::Pause);
    assert_eq!(world.read().step(), 0);
    for tick in 1..=3 {
        request(&control, Command::Step);
        assert_eq!(ticks.recv_timeout(TIMEOUT).unwrap(), tick);
        expected.tick().unwrap();
        assert_eq!(world.read().step(), tick);
        assert_eq!(*world.read(), *expected.universe().read());
        assert!(control.status().paused);
    }
    thread::sleep(Duration::from_millis(80));
    assert_eq!(world.read().step(), 3, "paused time must not advance ticks");
    request(&control, Command::Resume);
    assert!(ticks.recv_timeout(TIMEOUT).unwrap() > 3);
    request(&control, Command::Pause);
    let paused_at = world.read().step();
    request(&control, Command::Pause);
    assert_eq!(world.read().step(), paused_at);
    request(&control, Command::Step);
    assert_eq!(world.read().step(), paused_at + 1);
    request(&control, Command::Stop);
    let (actual, stop) = finished.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(stop, StopReason::Requested);
    while expected.universe().read().step() < actual.universe().read().step() {
        expected.tick().unwrap();
    }
    assert_eq!(actual, expected);
}

#[test]
fn zero_tick_limit_works_while_paused() {
    let mut run = Run::new(support::ecosystem("debug"), 1).unwrap();
    let original = run.clone();
    let (_control, commands) = Control::channel(true);
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let reason = run
            .execute(
                LoopOptions {
                    ticks: Some(0),
                    ..LoopOptions::default()
                },
                commands,
                |_, _| false,
            )
            .unwrap();
        done.send((run, reason)).unwrap();
    });
    let (run, reason) = finished.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(reason, StopReason::TickLimit);
    assert_eq!(run, original);
}

#[test]
fn dropping_last_control_wakes_a_paused_loop() {
    let mut run = Run::new(support::ecosystem("debug"), 1).unwrap();
    let original = run.clone();
    let (control, commands) = Control::channel(true);
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let reason = run
            .execute(LoopOptions::default(), commands, |_, _| false)
            .unwrap();
        done.send((run, reason)).unwrap();
    });
    request(&control, Command::Pause);
    drop(control);
    let (run, reason) = finished.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(reason, StopReason::Disconnected);
    assert_eq!(run, original);
}
