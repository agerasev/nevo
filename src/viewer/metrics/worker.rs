//! The UI exchanges only its latest request and a completed snapshot with this
//! worker. Loading and downsampling never hold an egui or world lock.

use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

use super::history::{History, Segment};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Request {
    pub metric: String,
    /// None requests the entire loaded history (fit/follow).
    pub range: Option<[f64; 2]>,
    pub pixels: usize,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            metric: String::new(),
            range: None,
            pixels: 1024,
        }
    }
}

#[derive(Default)]
pub(super) struct Snapshot {
    pub revision: u64,
    pub request: Request,
    pub names: Vec<String>,
    pub total_windows: u64,
    pub last_tick: Option<u64>,
    pub extent: Option<[u64; 2]>,
    pub pending: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub segments: Vec<Segment>,
}

#[derive(Default)]
struct Exchange {
    request: Request,
    snapshot: Arc<Snapshot>,
    stop: bool,
}

pub(super) struct Worker {
    shared: Arc<(Mutex<Exchange>, Condvar)>,
}

impl Worker {
    pub fn new(path: PathBuf) -> Self {
        let shared = Arc::new((Mutex::new(Exchange::default()), Condvar::new()));
        let worker = Self {
            shared: Arc::clone(&shared),
        };
        thread::Builder::new()
            .name("metrics-history".into())
            .spawn(move || run(path, shared))
            .expect("spawn metrics history reader");
        worker
    }

    pub fn request(&self, request: Request) {
        let (lock, wake) = &*self.shared;
        let mut exchange = lock.lock().unwrap();
        if exchange.request != request {
            exchange.request = request;
            wake.notify_one();
        }
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        Arc::clone(&self.shared.0.lock().unwrap().snapshot)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, wake) = &*self.shared;
        lock.lock().unwrap().stop = true;
        wake.notify_one();
        // Do not block window shutdown on a disk read. The worker exits after
        // its current batch/query and owns no simulation state.
    }
}

fn run(path: PathBuf, shared: Arc<(Mutex<Exchange>, Condvar)>) {
    let mut history = History::new(path);
    let mut last_request = None;
    let mut revision = 0;
    let mut dirty = true;
    let mut last_publish = Instant::now();
    let mut previous_error = None;
    let mut previous_pending = false;
    let mut previous_loading = true;
    let mut last_segments = Vec::new();
    loop {
        let (lock, wake) = &*shared;
        let request = {
            let exchange = lock.lock().unwrap();
            if exchange.stop {
                return;
            }
            exchange.request.clone()
        };
        let (loading, error) = match history.poll() {
            Ok(changed) => {
                dirty |= changed;
                (changed, None)
            }
            Err(error) => (false, Some(format!("{error:#}"))),
        };
        let pending = history.has_pending_record();
        let new_request = last_request.as_ref() != Some(&request);
        let status_changed =
            error != previous_error || pending != previous_pending || loading != previous_loading;
        // During the initial scan, publish periodically rather than querying
        // the growing history again after every small reader batch.
        let publish = new_request
            || status_changed
            || (dirty && (!loading || last_publish.elapsed() >= Duration::from_millis(100)));
        if publish {
            if (dirty || new_request) && !request.metric.is_empty() {
                last_segments = history.query(&request.metric, request.range, request.pixels);
            } else if request.metric.is_empty() {
                last_segments.clear();
            }
            revision += 1;
            let snapshot = Arc::new(Snapshot {
                revision,
                request: request.clone(),
                names: history.names().map(str::to_owned).collect(),
                total_windows: history.total_windows(),
                last_tick: history.last_tick(),
                extent: history.extent(),
                pending,
                loading,
                error: error.clone(),
                segments: last_segments.clone(),
            });
            let mut exchange = lock.lock().unwrap();
            if exchange.stop {
                return;
            }
            // A new request may arrive during downsampling. Never replace the
            // displayed result with a result for that superseded request.
            if exchange.request == request {
                exchange.snapshot = snapshot;
            }
            last_request = Some(request.clone());
            previous_error = error;
            previous_pending = pending;
            previous_loading = loading;
            last_publish = Instant::now();
            dirty = false;
        }
        if !loading {
            let exchange = lock.lock().unwrap();
            if exchange.stop {
                return;
            }
            if exchange.request == request {
                let _ = wake
                    .wait_timeout(exchange, Duration::from_millis(200))
                    .unwrap();
            }
        }
    }
}
