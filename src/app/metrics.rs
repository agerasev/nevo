//! Finalized window records; accumulators remain in their simulation components.

use std::path::Path;

use anyhow::Result;
use nevo::{
    metrics::{
        Metrics,
        log::{FORMAT_VERSION, Record, Session, Writer, create_invocation},
    },
    run::Run,
};

use super::{
    config::Config,
    persistence::{AGENT_MODEL, WORLD_MODEL},
};

pub struct MetricsLog {
    writer: Writer,
    session: u64,
    last_end_tick: Option<u64>,
}

impl MetricsLog {
    /// The caller holds the run's writer lock for this object's entire lifetime.
    /// Each invocation owns a fresh file; earlier logs stay untouched.
    pub fn new(directory: &Path, options: &Config, start_tick: u64) -> Result<Option<Self>> {
        if !options.ecosystem.metrics.enabled {
            return Ok(None);
        }
        let (path, mut writer) = create_invocation(directory)?;
        let session = writer.next_session_id()?;
        writer.append(&Record::Session(Session {
            version: FORMAT_VERSION,
            id: session,
            started_at: jiff::Timestamp::now().to_string(),
            start_tick,
            config: options.to_toml()?,
            code_version: format!("{} ({})", env!("CARGO_PKG_VERSION"), env!("NEVO_REVISION")),
            world_model: WORLD_MODEL.into(),
            mind_model: AGENT_MODEL.into(),
        }))?;
        eprintln!("metrics={}", path.display());
        Ok(Some(Self {
            writer,
            session,
            last_end_tick: None,
        }))
    }

    pub fn record(&mut self, run: &Run, include_partial: bool) -> Result<()> {
        let (end_tick, summary) = {
            let universe = run.universe().read();
            let world = universe.world();
            let Some(window) = world.metrics_window() else {
                return Ok(());
            };
            if window.samples == 0
                || (!include_partial && !window.complete)
                || self.last_end_tick == Some(window.end_tick)
            {
                return Ok(());
            }
            (window.end_tick, world.metrics())
        };
        // Encoding and disk I/O occur after releasing the universe read lock.
        self.writer
            .append_sample(self.session, end_tick, &summary)?;
        self.last_end_tick = Some(end_tick);
        Ok(())
    }

    pub fn finish(&mut self, run: &Run) -> Result<()> {
        self.record(run, true)?;
        self.writer.sync()
    }
}
