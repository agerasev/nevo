//! Run-directory storage; checkpoints are published only between complete ticks.

use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use nevo::{run::Run, universe::Universe};
use serde::{Deserialize, Serialize};

use super::{Counts, config::Config};

const FORMAT_VERSION: u32 = 6;
const WORLD_MODEL: &str = "foraging-sense-first-v1";
const AGENT_MODEL: &str = "animal-rnn-3-4-3-v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    format_version: u32,
    world_model: String,
    agent_model: String,
    pub config: Config,
    pub counts: Counts,
    pub state: Universe,
}

impl Checkpoint {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.format_version == FORMAT_VERSION,
            "unsupported checkpoint version {} (expected {FORMAT_VERSION})",
            self.format_version
        );
        ensure!(
            self.world_model == WORLD_MODEL,
            "unsupported world model {}",
            self.world_model
        );
        ensure!(
            self.agent_model == AGENT_MODEL,
            "unsupported agent model {}",
            self.agent_model
        );
        self.state.validate()
    }
}

/// The OS releases the advisory writer lock even after an interrupted process.
pub struct RunDirectory {
    path: PathBuf,
    _lock: File,
}

impl RunDirectory {
    pub fn open(requested: Option<&Path>) -> Result<Self> {
        let path = match requested {
            Some(path) => {
                fs::create_dir_all(path)
                    .with_context(|| format!("creating run directory {}", path.display()))?;
                path.to_owned()
            }
            None => {
                let root = Path::new("runs");
                fs::create_dir_all(root)?;
                let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                let mut attempt = 0_u64;
                loop {
                    let path = root.join(format!("run-{time}-{}-{attempt}", std::process::id()));
                    match fs::create_dir(&path) {
                        Ok(()) => break path,
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                            attempt += 1;
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
            }
        };
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.join(".lock"))?;
        lock.try_lock().with_context(|| {
            format!(
                "cannot lock run directory {} (another writer may be active)",
                path.display()
            )
        })?;
        let directory = Self { path, _lock: lock };
        fs::create_dir_all(directory.path.join("checkpoints"))?;
        Ok(directory)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn checkpoints(&self) -> Result<Vec<(u64, PathBuf)>> {
        let mut checkpoints = Vec::new();
        for entry in fs::read_dir(self.path.join("checkpoints"))? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(tick) = name.strip_suffix(".cbor") else {
                continue;
            };
            // Only our fixed-width checkpoint names participate in retention.
            ensure!(
                tick.len() == 20 && tick.bytes().all(|byte| byte.is_ascii_digit()),
                "invalid checkpoint filename {}",
                entry.path().display()
            );
            checkpoints.push((tick.parse()?, entry.path()));
        }
        checkpoints.sort_by_key(|(tick, _)| *tick);
        Ok(checkpoints)
    }

    pub fn load_latest(&self) -> Result<Option<Checkpoint>> {
        let Some((tick, path)) = self.checkpoints()?.pop() else {
            return Ok(None);
        };
        let mut reader = BufReader::new(File::open(&path)?);
        let checkpoint: Checkpoint = ciborium::from_reader(&mut reader).with_context(|| {
            format!(
                "reading checkpoint {} (only format {FORMAT_VERSION} is supported)",
                path.display()
            )
        })?;
        ensure!(
            reader.read(&mut [0])? == 0,
            "trailing data in checkpoint {}",
            path.display()
        );
        checkpoint.validate()?;
        ensure!(
            checkpoint.state.step() == tick,
            "checkpoint filename/tick mismatch"
        );
        ensure!(
            checkpoint.config.seed.is_some(),
            "checkpoint has no resolved seed"
        );
        Ok(Some(checkpoint))
    }

    pub fn save_config(&self, options: &Config) -> Result<()> {
        let text = options.to_toml()?;
        atomic_write(&self.path.join("config.toml"), |writer| {
            writer.write_all(text.as_bytes())?;
            Ok(())
        })
    }

    pub fn save(&self, run: &Run, options: &Config, counts: &Counts) -> Result<()> {
        ensure!(
            options.keep_checkpoints > 0,
            "keep-checkpoints must be positive"
        );
        let checkpoint = Checkpoint {
            format_version: FORMAT_VERSION,
            world_model: WORLD_MODEL.into(),
            agent_model: AGENT_MODEL.into(),
            config: options.clone(),
            counts: counts.clone(),
            state: run.universe().read().clone(),
        };
        let tick = checkpoint.state.step();
        let path = self
            .path
            .join("checkpoints")
            .join(format!("{tick:020}.cbor"));
        atomic_write(&path, |writer| {
            ciborium::into_writer(&checkpoint, writer).context("encoding checkpoint")
        })?;
        // Publish the new checkpoint before removing any older recoverable state.
        let checkpoints = self.checkpoints()?;
        let excess = checkpoints.len().saturating_sub(options.keep_checkpoints);
        for (_, old) in checkpoints.into_iter().take(excess) {
            fs::remove_file(&old)
                .with_context(|| format!("removing old checkpoint {}", old.display()))?;
        }
        sync_directory(&self.path.join("checkpoints"))?;
        eprintln!("checkpoint={}", path.display());
        Ok(())
    }

    /// Do not silently start fresh in a directory containing an unrelated file.
    pub fn check_new(&self) -> Result<()> {
        for entry in fs::read_dir(&self.path)? {
            let entry = entry?;
            if !matches!(
                entry.file_name().to_str(),
                Some(".lock" | "checkpoints" | "config.toml" | ".config.toml.tmp")
            ) {
                bail!(
                    "run directory contains an unrecognized file: {}",
                    entry.path().display()
                );
            }
        }
        Ok(())
    }
}

fn atomic_write(path: &Path, write: impl FnOnce(&mut BufWriter<File>) -> Result<()>) -> Result<()> {
    let parent = path.parent().context("output path has no parent")?;
    let name = path
        .file_name()
        .context("output path has no name")?
        .to_string_lossy();
    // A single writer holds the directory lock. Incomplete temporary files are
    // ignored by discovery and safely replaced after a crash.
    let temporary = parent.join(format!(".{name}.tmp"));
    let result: Result<()> = (|| {
        let mut writer = BufWriter::new(File::create(&temporary)?);
        write(&mut writer)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        drop(writer);
        fs::rename(&temporary, path)?;
        sync_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("saving {}", path.display()))
}

fn sync_directory(path: &Path) -> Result<()> {
    // Unix needs the directory entry synced as well as the file contents.
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_version_and_models_are_checked_before_restore() {
        let checkpoint = Checkpoint {
            format_version: FORMAT_VERSION,
            world_model: WORLD_MODEL.into(),
            agent_model: AGENT_MODEL.into(),
            config: Config {
                seed: Some(1),
                ..Config::default()
            },
            counts: Counts::default(),
            state: Universe::new(nevo::ecosystem::Config::default(), 1).unwrap(),
        };
        for (name, invalid) in [
            ("format_version", ciborium::Value::from(3)),
            ("format_version", ciborium::Value::from(99)),
            ("world_model", ciborium::Value::from("another-world")),
            ("agent_model", ciborium::Value::from("another-agent")),
        ] {
            let mut value = ciborium::Value::serialized(&checkpoint).unwrap();
            value
                .as_map_mut()
                .unwrap()
                .iter_mut()
                .find(|(key, _)| key.as_text() == Some(name))
                .unwrap()
                .1 = invalid;
            let invalid: Checkpoint = value.deserialized().unwrap();
            assert!(invalid.validate().is_err(), "accepted invalid {name}");
        }
    }

    #[test]
    fn failed_publication_preserves_the_previous_checkpoint() {
        let path = std::env::temp_dir().join(format!("nevo-save-failure-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let directory = RunDirectory::open(Some(&path)).unwrap();
        let options = Config {
            seed: Some(1),
            keep_checkpoints: 1,
            ..Config::default()
        };
        let mut run = Run::new(nevo::ecosystem::Config::default(), 1).unwrap();
        directory.save(&run, &options, &Counts::default()).unwrap();
        let previous = path.join("checkpoints/00000000000000000000.cbor");
        let bytes = fs::read(&previous).unwrap();
        run.tick().unwrap();
        let blocked = path.join("checkpoints/00000000000000000001.cbor");
        fs::create_dir(&blocked).unwrap();
        assert!(directory.save(&run, &options, &Counts::default()).is_err());
        assert_eq!(fs::read(previous).unwrap(), bytes);
        assert!(
            !path
                .join("checkpoints/.00000000000000000001.cbor.tmp")
                .exists()
        );
        drop(directory);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn interrupted_encoding_preserves_the_destination() {
        let path = std::env::temp_dir().join(format!("nevo-atomic-failure-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        let file = path.join("checkpoint.cbor");
        fs::write(&file, b"previous").unwrap();
        let result = atomic_write(&file, |writer| {
            writer.write_all(b"incomplete")?;
            bail!("encoding failed")
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"previous");
        assert!(!path.join(".checkpoint.cbor.tmp").exists());
        fs::remove_dir_all(path).unwrap();
    }
}
