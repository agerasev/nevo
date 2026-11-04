//! Finalized window summaries; no accumulators or simulation state live here.

use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result};
use nevo::{metrics::Metrics, run::Run};

pub struct MetricsLog {
    writer: BufWriter<File>,
    last_end_tick: Option<u64>,
}

impl MetricsLog {
    pub fn new(directory: &Path, enabled: bool) -> Result<Option<Self>> {
        if !enabled {
            return Ok(None);
        }
        let directory = directory.join("metrics");
        fs::create_dir_all(&directory).context("creating metrics directory")?;
        let name = format!(
            "{}-{}",
            jiff::Zoned::now().strftime("%Y-%m-%d_%H-%M-%S"),
            std::process::id()
        );
        let mut attempt = 1_u64;
        let (path, file) = loop {
            let filename = if attempt == 1 {
                format!("{name}.jsonl")
            } else {
                format!("{name}-{attempt}.jsonl")
            };
            let path = directory.join(filename);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => break (path, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => attempt += 1,
                Err(error) => return Err(error).context("creating metrics log"),
            }
        };
        eprintln!("metrics={}", path.display());
        Ok(Some(Self {
            writer: BufWriter::new(file),
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
        // Serialization and disk I/O must not hold up readers of the universe.
        let encoded = serde_json::to_vec(&summary).context("encoding metrics summary")?;
        self.writer.write_all(&encoded)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush().context("flushing metrics summary")?;
        self.last_end_tick = Some(end_tick);
        Ok(())
    }
}
