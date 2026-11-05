//! Append-only CBOR sequences shared by recording and read-only observation.
//!
//! Each invocation appends a Session record, then Sample records. Readers always
//! follow growth; an incomplete final item is pending, while malformed data is
//! an error. The caller holds the run's writer lock when opening a Writer.

mod reader;
mod writer;

use anyhow::{Result, ensure};
use ciborium::Value;
use serde::{Deserialize, Serialize};

pub use reader::Reader;
pub use writer::Writer;

pub const FILE_NAME: &str = "metrics.cborseq";
pub const FORMAT_VERSION: u32 = 1;
/// Bounds memory and computation for one externally supplied record.
pub const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    Session(Session),
    Sample(Sample),
}

/// Immutable provenance for one invocation, stored in the same file as samples.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub version: u32,
    pub id: u64,
    pub started_at: String,
    pub start_tick: u64,
    /// The exact effective TOML configuration, independent of later overrides.
    pub config: String,
    pub code_version: String,
    pub world_model: String,
    pub mind_model: String,
}

/// Finalized component metrics. Simulation-specific fields stay in `values`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub session: u64,
    /// Window end tick; overlapping tick ranges across sessions remain distinct.
    pub tick: u64,
    pub values: Value,
}

#[derive(Clone, Debug, Default)]
struct Position {
    session: u64,
    tick: u64,
}

impl Position {
    fn next_session_id(&self) -> Result<u64> {
        self.session
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("metrics session counter exhausted"))
    }

    fn observe(&mut self, record: &Record) -> Result<()> {
        match record {
            Record::Session(session) => {
                ensure!(
                    session.version == FORMAT_VERSION,
                    "unsupported metrics format version {} (expected {FORMAT_VERSION})",
                    session.version
                );
                ensure!(
                    session.id == self.next_session_id()?,
                    "metrics session IDs must increase consecutively"
                );
                self.session = session.id;
                self.tick = session.start_tick;
            }
            Record::Sample(sample) => {
                ensure!(
                    self.session != 0 && sample.session == self.session,
                    "metrics sample does not belong to the current session"
                );
                ensure!(
                    sample.tick > self.tick,
                    "metrics sample ticks must increase within a session"
                );
                self.tick = sample.tick;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
