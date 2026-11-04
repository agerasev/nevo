//! Simulation stepping and execution controls.

use std::{
    fmt,
    sync::{Arc, Mutex, RwLock, RwLockReadGuard, mpsc},
    time::{Duration, Instant},
};

use crate::{
    ecosystem::{Config, Ecosystem},
    universe::{Simulation, Universe},
};
use anyhow::Result;

/// Read-only access to complete state. Cloning attaches another reader.
pub struct UniverseView<U = Universe>(Arc<RwLock<U>>);

impl<U> Clone for UniverseView<U> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<U> UniverseView<U> {
    pub fn read(&self) -> RwLockReadGuard<'_, U> {
        self.0.read().expect("universe publication panicked")
    }
}

impl<U: fmt::Debug> fmt::Debug for UniverseView<U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.read().fmt(f)
    }
}

/// The sole writer applies complete updates; observers only receive readers.
#[derive(Debug)]
pub struct Run<S = Ecosystem> {
    universe: UniverseView<Universe<S>>,
}

impl<S> Run<S> {
    pub fn universe(&self) -> &UniverseView<Universe<S>> {
        &self.universe
    }

    pub fn from_universe(universe: Universe<S>) -> Self {
        Self {
            universe: UniverseView(Arc::new(RwLock::new(universe))),
        }
    }
}

/// Cloning a run makes an independent simulation, unlike cloning its view.
impl<S: Clone> Clone for Run<S> {
    fn clone(&self) -> Self {
        Self::from_universe(self.universe.read().clone())
    }
}

impl<S: PartialEq> PartialEq for Run<S> {
    fn eq(&self, other: &Self) -> bool {
        *self.universe.read() == *other.universe.read()
    }
}

impl Run {
    pub fn new(config: Config, seed: u64) -> Result<Self> {
        Ok(Self::from_universe(Universe::new(config, seed)?))
    }
}

/// Limits apply to this invocation; completing a loop never resets the run.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoopOptions {
    pub ticks: Option<u64>,
    /// Defaults to zero: run as fast as possible. Pacing never changes the timestep.
    pub interval: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Pause,
    Resume,
    TogglePause,
    Step,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    TickLimit,
    Condition,
    Requested,
    Disconnected,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoopStatus {
    pub paused: bool,
    pub steps: u64,
    pub stop: Option<StopReason>,
}

struct Request {
    command: Command,
    receipt: Option<mpsc::Sender<()>>,
}

impl Request {
    fn acknowledge(self) {
        if let Some(receipt) = self.receipt {
            let _ = receipt.send(());
        }
    }
}

/// Commands are applied between complete ticks. Readers cannot mutate the universe.
#[derive(Clone)]
pub struct Control {
    sender: mpsc::Sender<Request>,
    status: Arc<Mutex<LoopStatus>>,
}

pub struct Commands {
    receiver: mpsc::Receiver<Request>,
    status: Arc<Mutex<LoopStatus>>,
}

impl Control {
    pub fn channel(paused: bool) -> (Self, Commands) {
        let (sender, receiver) = mpsc::channel();
        let status = Arc::new(Mutex::new(LoopStatus {
            paused,
            steps: 0,
            stop: None,
        }));
        (
            Self {
                sender,
                status: Arc::clone(&status),
            },
            Commands { receiver, status },
        )
    }

    pub fn status(&self) -> LoopStatus {
        *self.status.lock().expect("loop status poisoned")
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.sender
            .send(Request {
                command,
                receipt: None,
            })
            .map_err(|_| "simulation loop has stopped".into())
    }

    /// Wait for a boundary acknowledgement; Step completes its tick first.
    /// Call from a controller thread, never from the simulation's own callback.
    pub fn request(&self, command: Command) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(Request {
                command,
                receipt: Some(sender),
            })
            .map_err(|_| "simulation loop has stopped".to_owned())?;
        receiver
            .recv()
            .map_err(|_| "simulation stopped before acknowledging command".into())
    }
}

impl<S> Run<S>
where
    S: Simulation,
{
    pub fn tick(&mut self) -> Result<S::Report> {
        // Keep the read lock for preparation: viewers may read the old complete
        // state concurrently. Drop it before acquiring the publication lock.
        let update = self.universe.read().prepare_step()?;
        let report = self
            .universe
            .0
            .write()
            .expect("universe publication panicked")
            .apply_step(update);
        Ok(report)
    }

    /// The single execution loop, with optional limits, pacing and external controls.
    /// `after_tick` runs without a universe lock and returns true to stop.
    pub fn execute(
        &mut self,
        options: LoopOptions,
        commands: Commands,
        mut after_tick: impl FnMut(&Self, &S::Report) -> bool,
    ) -> Result<StopReason> {
        let mut last_tick = Instant::now();
        let mut status = *commands.status.lock().expect("loop status poisoned");
        status.steps = 0;
        status.stop = None;
        let publish = |state| *commands.status.lock().expect("loop status poisoned") = state;
        let limit = |steps| {
            options
                .ticks
                .is_some_and(|ticks| steps >= ticks)
                .then_some(StopReason::TickLimit)
        };
        publish(status);
        loop {
            if let Some(reason) = limit(status.steps) {
                status.stop = Some(reason);
                publish(status);
                return Ok(reason);
            }
            let request = match commands.receiver.try_recv() {
                Ok(request) => Some(request),
                Err(mpsc::TryRecvError::Disconnected) => {
                    status.stop = Some(StopReason::Disconnected);
                    publish(status);
                    return Ok(StopReason::Disconnected);
                }
                Err(mpsc::TryRecvError::Empty) => {
                    let until_tick = (!status.paused)
                        .then(|| options.interval.saturating_sub(last_tick.elapsed()));
                    if until_tick == Some(Duration::ZERO) {
                        None
                    } else {
                        let received = match until_tick {
                            Some(wait) => commands.receiver.recv_timeout(wait),
                            None => commands
                                .receiver
                                .recv()
                                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
                        };
                        match received {
                            Ok(request) => Some(request),
                            Err(mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(mpsc::RecvTimeoutError::Disconnected) => {
                                status.stop = Some(StopReason::Disconnected);
                                publish(status);
                                return Ok(StopReason::Disconnected);
                            }
                        }
                    }
                }
            };

            if let Some(command) = request.as_ref().map(|request| request.command) {
                match command {
                    Command::Pause | Command::Step => status.paused = true,
                    Command::Resume => status.paused = false,
                    Command::TogglePause => status.paused = !status.paused,
                    Command::Stop => status.stop = Some(StopReason::Requested),
                }
                last_tick = Instant::now();
                publish(status);
                if command != Command::Step {
                    request.expect("request is present").acknowledge();
                    if let Some(reason) = status.stop {
                        return Ok(reason);
                    }
                    continue;
                }
            }

            let report = match self.tick() {
                Ok(report) => report,
                Err(error) => {
                    status.stop = Some(StopReason::Failed);
                    publish(status);
                    return Err(error);
                }
            };
            status.steps += 1;
            last_tick = Instant::now();
            status.stop = if after_tick(self, &report) {
                Some(StopReason::Condition)
            } else {
                limit(status.steps)
            };
            publish(status);
            if let Some(request) = request {
                request.acknowledge();
            }
            if let Some(reason) = status.stop {
                return Ok(reason);
            }
        }
    }
}

#[cfg(test)]
mod tests;
