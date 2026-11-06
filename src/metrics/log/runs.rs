//! One new file per invocation; old single-file histories remain read-only.

use std::{
    collections::{BTreeSet, VecDeque},
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};

use super::{FILE_NAME, Reader, Record, Writer};

pub const DIRECTORY_NAME: &str = "metrics";

fn files(directory: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("listing metrics files"),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "cborseq") {
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths)
}

/// The caller holds the run writer lock. Earlier files are never opened for writing.
pub fn create_invocation(run: &Path) -> Result<(PathBuf, Writer)> {
    let directory = run.join(DIRECTORY_NAME);
    fs::create_dir_all(&directory)?;
    let mut last = 0_u64;
    for path in files(&directory)? {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let id: u64 = stem
            .parse()
            .context("invalid metrics invocation filename")?;
        ensure!(
            stem == format!("{id:020}"),
            "invalid metrics invocation filename"
        );
        last = last.max(id);
    }
    let id = last
        .checked_add(1)
        .context("metrics file counter exhausted")?;
    let path = directory.join(format!("{id:020}.cborseq"));
    let writer = Writer::create(&path)?;
    Ok((path, writer))
}

/// Follow the legacy log, then immutable earlier invocation files and the growing
/// newest file. IDs are assigned in reading order so each file has its own curve.
pub struct RunReader {
    run: PathBuf,
    known: BTreeSet<PathBuf>,
    queued: VecDeque<PathBuf>,
    current: Option<Reader>,
    session: u64,
}

impl RunReader {
    pub fn new(run: impl AsRef<Path>) -> Self {
        Self {
            run: run.as_ref().to_owned(),
            known: BTreeSet::new(),
            queued: VecDeque::new(),
            current: None,
            session: 0,
        }
    }

    pub fn has_pending_record(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(Reader::has_pending_record)
    }

    pub fn poll(&mut self) -> Result<Vec<Record>> {
        // A file path remains usable for standalone/archived log inspection.
        let paths =
            if !self.run.is_dir() && self.run.extension().is_some_and(|ext| ext == "cborseq") {
                vec![self.run.clone()]
            } else {
                let legacy = self.run.join(FILE_NAME);
                let mut paths = Vec::new();
                if legacy.exists() {
                    paths.push(legacy);
                }
                paths.extend(files(&self.run.join(DIRECTORY_NAME))?);
                paths
            };
        for path in paths {
            if self.known.insert(path.clone()) {
                self.queued.push_back(path);
            }
        }
        // Bound work even with many empty/interrupted invocations.
        for _ in 0..16 {
            if self.current.is_none() {
                self.current = self.queued.pop_front().map(Reader::new);
            }
            let Some(reader) = &mut self.current else {
                return Ok(Vec::new());
            };
            let mut records = reader.poll()?;
            if !records.is_empty() {
                for record in &mut records {
                    match record {
                        Record::Session(header) => {
                            self.session = self
                                .session
                                .checked_add(1)
                                .context("metrics session counter exhausted")?;
                            header.id = self.session;
                        }
                        Record::Sample(sample) => sample.session = self.session,
                    }
                }
                return Ok(records);
            }
            if self.queued.is_empty() {
                break;
            }
            // A crash may leave a partial tail in an older file. Keep it intact;
            // a later invocation's existence proves no more bytes will arrive.
            self.current = None;
        }
        Ok(Vec::new())
    }
}
