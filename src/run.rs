use std::{
    fmt,
    sync::{Arc, Mutex, RwLock, RwLockReadGuard, mpsc},
    time::{Duration, Instant},
};

use rand::{Rng, SeedableRng};
use rand_xoshiro::SplitMix64;

use crate::{
    AgentResponse, Environment, Population,
    foraging::{ForagingWorld, Scenario},
    herbivore::Herbivores,
};

/// Read-only access to a live world. Cloning this handle attaches another reader.
pub struct WorldView<W>(Arc<RwLock<W>>);

impl<W> Clone for WorldView<W> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<W> WorldView<W> {
    pub fn read(&self) -> RwLockReadGuard<'_, W> {
        self.0.read().expect("world update panicked")
    }
}

impl<W: fmt::Debug> fmt::Debug for WorldView<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.read().fmt(f)
    }
}

/// The sole writer owns agent state; observers can only read the shared world.
#[derive(Debug)]
pub struct Run<W = ForagingWorld, P = Herbivores> {
    world: WorldView<W>,
    pub population: P,
}

impl<W, P> Run<W, P> {
    pub fn world(&self) -> &WorldView<W> {
        &self.world
    }

    pub fn from_parts(world: W, population: P) -> Self {
        Self {
            world: WorldView(Arc::new(RwLock::new(world))),
            population,
        }
    }
}

/// Cloning a run makes an independent simulation, unlike cloning its world view.
impl<W: Clone, P: Clone> Clone for Run<W, P> {
    fn clone(&self) -> Self {
        Self::from_parts(self.world.read().clone(), self.population.clone())
    }
}

impl<W: PartialEq, P: PartialEq> PartialEq for Run<W, P> {
    fn eq(&self, other: &Self) -> bool {
        *self.world.read() == *other.world.read() && self.population == other.population
    }
}

impl Run {
    pub fn new(scenario: Scenario, seed: u64) -> Self {
        let mut seeds = SplitMix64::seed_from_u64(seed);
        let world_seed = seeds.next_u64();
        let mut population = Herbivores::new(seeds.next_u64());
        let world = ForagingWorld::new(scenario, world_seed, &mut population);
        Self::from_parts(world, population)
    }
}

/// Limits apply to this invocation; completing a loop never resets the run.
#[derive(Clone, Copy, Debug)]
pub struct LoopOptions {
    pub ticks: Option<u64>,
    pub duration: Option<Duration>,
    /// Zero runs as fast as possible. Pacing never changes the simulation timestep.
    pub interval: Duration,
}

impl Default for LoopOptions {
    fn default() -> Self {
        Self {
            ticks: None,
            duration: None,
            interval: Duration::from_millis(40),
        }
    }
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
    TimeLimit,
    Condition,
    Requested,
    Disconnected,
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

/// Commands are applied between complete ticks. Readers cannot mutate the world.
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

impl<W, P> Run<W, P>
where
    W: Environment<P>,
    P: Population<W::Observation, W::Action, W::Origin>,
{
    pub fn tick(&mut self) -> W::Report {
        let observations = self.world.read().observe();
        // The read guard is gone before agent processing; observers remain free to read.
        let responses = observations
            .into_iter()
            .map(|(id, observation)| AgentResponse {
                id,
                action: self.population.act(id, &observation),
                observation,
            })
            .collect();
        self.world
            .0
            .write()
            .expect("world update panicked")
            .update(&mut self.population, responses)
    }

    /// The single execution loop, with optional limits, pacing and external controls.
    /// `after_tick` runs without a world lock and returns true to stop.
    pub fn execute(
        &mut self,
        options: LoopOptions,
        commands: Commands,
        mut after_tick: impl FnMut(&Self, &W::Report) -> bool,
    ) -> StopReason {
        let started = Instant::now();
        let mut last_tick = started;
        let mut status = *commands.status.lock().expect("loop status poisoned");
        status.steps = 0;
        status.stop = None;
        let publish = |state| *commands.status.lock().expect("loop status poisoned") = state;
        let limit = |steps| {
            if options.ticks.is_some_and(|ticks| steps >= ticks) {
                Some(StopReason::TickLimit)
            } else if options
                .duration
                .is_some_and(|time| started.elapsed() >= time)
            {
                Some(StopReason::TimeLimit)
            } else {
                None
            }
        };
        publish(status);
        loop {
            if let Some(reason) = limit(status.steps) {
                status.stop = Some(reason);
                publish(status);
                return reason;
            }
            let request = match commands.receiver.try_recv() {
                Ok(request) => Some(request),
                Err(mpsc::TryRecvError::Disconnected) => {
                    status.stop = Some(StopReason::Disconnected);
                    publish(status);
                    return StopReason::Disconnected;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    let until_tick = (!status.paused)
                        .then(|| options.interval.saturating_sub(last_tick.elapsed()));
                    if until_tick == Some(Duration::ZERO) {
                        None
                    } else {
                        let until_limit = options
                            .duration
                            .map(|duration| duration.saturating_sub(started.elapsed()));
                        let wait = until_tick.into_iter().chain(until_limit).min();
                        let received = match wait {
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
                                return StopReason::Disconnected;
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
                        return reason;
                    }
                    continue;
                }
            }

            // A command may have arrived before a deadline but been scheduled after it.
            if let Some(reason) = limit(status.steps) {
                status.stop = Some(reason);
                publish(status);
                return reason;
            }
            let report = self.tick();
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
                return reason;
            }
        }
    }
}

#[cfg(test)]
mod tests;
