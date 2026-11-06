use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
};

use anyhow::{Context, Result, ensure};
use ciborium::Value;
use serde::Serialize;

use super::{MAX_RECORD_BYTES, Position, Reader, Record, Sample};

/// The caller exclusively owns the run directory's writer lock for this lifetime.
/// Each record reaches the file before `append` returns; `sync` requests durability.
pub struct Writer {
    file: File,
    position: Position,
    poisoned: bool,
}

impl Writer {
    /// Create one invocation without modifying any existing log.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .context("creating metrics invocation file")?;
        Ok(Self {
            file,
            position: Position::default(),
            poisoned: false,
        })
    }

    /// Validate all existing records before changing the file. Only an incomplete
    /// final CBOR item is removed; malformed CBOR/schema/sequence data is an error.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut reader = Reader::new(path);
        while !reader.poll()?.is_empty() {}
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)
            .context("opening metrics log for appending")?;
        if reader.has_pending_record() {
            file.set_len(reader.offset())
                .context("removing incomplete final metrics record")?;
        }
        Ok(Self {
            file,
            position: reader.position().clone(),
            poisoned: false,
        })
    }

    pub fn next_session_id(&self) -> Result<u64> {
        self.position.next_session_id()
    }

    pub fn append_sample<T: Serialize>(
        &mut self,
        session: u64,
        tick: u64,
        values: &T,
    ) -> Result<()> {
        self.append(&Record::Sample(Sample {
            session,
            tick,
            values: Value::serialized(values).context("encoding metrics values")?,
        }))
    }

    pub fn append(&mut self, record: &Record) -> Result<()> {
        ensure!(
            !self.poisoned,
            "metrics writer failed previously; reopen it to recover the trailing record"
        );
        let mut position = self.position.clone();
        position.observe(record)?;
        let mut encoded = Vec::new();
        ciborium::into_writer(record, &mut encoded).context("encoding metrics record")?;
        ensure!(
            encoded.len() as u64 <= MAX_RECORD_BYTES,
            "metrics record exceeds {MAX_RECORD_BYTES} bytes"
        );
        if let Err(error) = self.write_record(&encoded) {
            // A short write may leave a partial tail. No later record may follow
            // that tail until open() validates and repairs it under the writer lock.
            self.poisoned = true;
            return Err(error).context("appending metrics record");
        }
        self.position = position;
        Ok(())
    }

    fn write_record(&mut self, encoded: &[u8]) -> io::Result<()> {
        self.file.write_all(encoded)?;
        self.file.flush()
    }

    pub fn sync(&self) -> Result<()> {
        self.file.sync_data().context("syncing metrics log")
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::*;
    use crate::metrics::log::{FORMAT_VERSION, Session};

    #[test]
    fn an_io_failure_poison_requires_reopening_even_after_the_file_is_writable() {
        let path = std::env::temp_dir().join(format!(
            "nevo-metrics-write-failure-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut writer = Writer::open(&path).unwrap();
        writer
            .append(&Record::Session(Session {
                version: FORMAT_VERSION,
                id: 1,
                started_at: "2026-11-05T00:00:00Z".into(),
                start_tick: 0,
                config: "seed = 42\n".into(),
                code_version: "test".into(),
                world_model: "test-world".into(),
                mind_model: "test-mind".into(),
            }))
            .unwrap();
        let before = fs::read(&path).unwrap();
        writer.file = File::open(&path).unwrap(); // Deliberately read-only.
        let error = writer.append_sample(1, 1, &42_u64).unwrap_err();
        assert!(error.to_string().contains("appending metrics"));
        assert!(writer.poisoned);
        assert_eq!(writer.position.tick, 0);

        writer.file = OpenOptions::new().append(true).open(&path).unwrap();
        let error = writer.append_sample(1, 1, &42_u64).unwrap_err();
        assert!(error.to_string().contains("failed previously"));
        assert_eq!(fs::read(&path).unwrap(), before);
        drop(writer);

        let mut reopened = Writer::open(&path).unwrap();
        reopened.append_sample(1, 1, &42_u64).unwrap();
        let mut reader = Reader::new(&path);
        assert_eq!(reader.poll().unwrap().len(), 2);
        assert!(reader.poll().unwrap().is_empty());
        drop(reopened);
        fs::remove_file(path).unwrap();
    }
}
