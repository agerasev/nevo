use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    ops::Range,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use ciborium::Value;

use super::{MAX_RECORD_BYTES, Position, Record};

const POLL_RECORDS: usize = 128;
const POLL_BYTES: u64 = 1024 * 1024;

/// A validated record and its exact byte range, suitable for a rebuildable index.
#[derive(Clone, Debug)]
pub struct LocatedRecord {
    pub record: Record,
    pub offset: u64,
    pub end_offset: u64,
}

/// Incrementally follows one file, including creation after the reader starts.
/// `poll` never finishes permanently: an empty result means caught up for now.
/// A poll decodes at most 128 records or about 1 MiB (plus one bounded record).
/// A valid prefix is returned before an error in a later item; the following poll
/// reports that error without consuming the offending bytes.
pub struct Reader {
    path: PathBuf,
    offset: u64,
    position: Position,
    pending: bool,
}

impl Reader {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            offset: 0,
            position: Position::default(),
            pending: false,
        }
    }

    /// Offset after the last published complete record, never inside a pending item.
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn has_pending_record(&self) -> bool {
        self.pending
    }

    pub fn poll(&mut self) -> Result<Vec<Record>> {
        Ok(self
            .poll_located()?
            .into_iter()
            .map(|item| item.record)
            .collect())
    }

    /// Like `poll`, retaining the byte locations for subsequent range reads.
    pub fn poll_located(&mut self) -> Result<Vec<LocatedRecord>> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound && self.offset == 0 => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error).context("opening metrics log for reading"),
        };
        let length = file.metadata().context("reading metrics log size")?.len();
        ensure!(
            length >= self.offset,
            "metrics log was truncated before the last complete record"
        );
        let mut input = BufReader::new(file);
        input.seek(SeekFrom::Start(self.offset))?;
        let mut position = self.position.clone();
        let mut offset = self.offset;
        let mut pending = false;
        let mut records = Vec::new();
        while offset < length && records.len() < POLL_RECORDS {
            let decoded = decode(&mut input, length - offset, offset).and_then(|item| {
                if let Some((record, _)) = &item {
                    position
                        .observe(record)
                        .with_context(|| format!("invalid metrics sequence at offset {offset}"))?;
                }
                Ok(item)
            });
            match decoded {
                Ok(Some((record, consumed))) => {
                    records.push(LocatedRecord {
                        record,
                        offset,
                        end_offset: offset + consumed,
                    });
                    offset += consumed;
                }
                Ok(None) => {
                    pending = true;
                    break;
                }
                Err(error) if records.is_empty() => return Err(error),
                // Deliver the valid history first. The cursor remains before the
                // invalid item, so the next poll reports its error immediately.
                Err(_) => break,
            }
            if offset - self.offset >= POLL_BYTES {
                break;
            }
        }
        // A failed poll leaves both the cursor and sequence validation unchanged.
        self.offset = offset;
        self.position = position;
        self.pending = pending;
        Ok(records)
    }

    pub(super) fn position(&self) -> &Position {
        &self.position
    }
}

/// Read an already indexed range whose endpoints are complete record boundaries.
/// This checks CBOR/schema/size, but sequence ordering was checked by `Reader`
/// when building the index. Appends beyond the requested end are not read.
pub fn read_range(path: impl AsRef<Path>, range: Range<u64>) -> Result<Vec<Record>> {
    ensure!(range.start <= range.end, "invalid metrics byte range");
    let file = File::open(path).context("opening indexed metrics log")?;
    ensure!(
        file.metadata()?.len() >= range.end,
        "metrics log was truncated before the indexed range"
    );
    let mut input = BufReader::new(file);
    input.seek(SeekFrom::Start(range.start))?;
    let mut offset = range.start;
    let mut records = Vec::new();
    while offset < range.end {
        let Some((record, consumed)) = decode(&mut input, range.end - offset, offset)? else {
            bail!("incomplete metrics record in indexed range at offset {offset}");
        };
        records.push(record);
        offset += consumed;
    }
    Ok(records)
}

/// Freeze the visible byte range for this poll. An incomplete item is retried
/// from its beginning next time, including after the writer repairs its tail.
fn decode(input: &mut impl Read, available: u64, offset: u64) -> Result<Option<(Record, u64)>> {
    let limit = available.min(MAX_RECORD_BYTES + 1);
    let mut item = input.take(limit);
    let decoded: std::result::Result<Value, _> = ciborium::from_reader(&mut item);
    let consumed = limit - item.limit();
    ensure!(
        consumed <= MAX_RECORD_BYTES,
        "metrics record exceeds {MAX_RECORD_BYTES} bytes at offset {offset}"
    );
    let value = match decoded {
        Ok(value) => value,
        Err(ciborium::de::Error::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
            ensure!(
                available <= MAX_RECORD_BYTES,
                "metrics record exceeds {MAX_RECORD_BYTES} bytes at offset {offset}"
            );
            return Ok(None);
        }
        Err(error) => bail!("malformed metrics CBOR at offset {offset}: {error}"),
    };
    // Decode the complete CBOR item before interpreting its schema. An invalid
    // complete record must never be mistaken for an unfinished tail.
    let record = value
        .deserialized()
        .with_context(|| format!("invalid metrics record at offset {offset}"))?;
    Ok(Some((record, consumed)))
}
