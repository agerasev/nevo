use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nevo-metrics-log-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join(FILE_NAME)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn session(id: u64, start_tick: u64) -> Record {
    Record::Session(Session {
        version: FORMAT_VERSION,
        id,
        started_at: "2026-11-05T00:00:00Z".into(),
        start_tick,
        config: "seed = 42\n".into(),
        code_version: "test".into(),
        world_model: "test-world".into(),
        mind_model: "test-mind".into(),
    })
}

fn sample(session: u64, tick: u64) -> Record {
    Record::Sample(Sample {
        session,
        tick,
        values: Value::Map(vec![
            (Value::Text("population".into()), Value::from(17)),
            (Value::Text("energy".into()), Value::Float(213.0)),
        ]),
    })
}

fn encoded(record: &Record) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(record, &mut bytes).unwrap();
    bytes
}

fn append_raw(path: &PathBuf, bytes: &[u8]) {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

fn drain(reader: &mut Reader) -> Vec<Record> {
    let mut all = Vec::new();
    loop {
        let records = reader.poll().unwrap();
        if records.is_empty() {
            return all;
        }
        all.extend(records);
    }
}

#[test]
fn follows_creation_and_every_byte_of_new_records_without_duplicates() {
    let directory = Directory::new();
    let path = directory.file();
    let mut reader = Reader::new(&path);
    assert!(reader.poll().unwrap().is_empty());
    assert!(!path.exists(), "observing must not create the log");
    let records = [
        session(1, 0),
        sample(1, 1),
        sample(1, 3),
        session(2, 1),
        sample(2, 2),
    ];
    for record in records {
        let bytes = encoded(&record);
        for (index, byte) in bytes.iter().enumerate() {
            append_raw(&path, &[*byte]);
            let found = reader.poll().unwrap();
            if index + 1 == bytes.len() {
                assert_eq!(found.as_slice(), std::slice::from_ref(&record));
                assert!(!reader.has_pending_record());
            } else {
                assert!(found.is_empty());
                assert!(reader.has_pending_record());
            }
        }
        assert!(
            reader.poll().unwrap().is_empty(),
            "EOF remains a waiting state"
        );
    }
}

#[test]
fn malformed_complete_records_and_invalid_sequences_are_never_truncated() {
    let directory = Directory::new();
    let path = directory.file();
    let prefix = encoded(&session(1, 0));
    let mut wrong_version = session(2, 0);
    if let Record::Session(value) = &mut wrong_version {
        value.version += 1;
    }
    let invalid = [
        vec![0xff], // Break outside an indefinite container: malformed CBOR.
        vec![0x01], // Complete CBOR, wrong record schema.
        encoded(&sample(2, 1)),
        encoded(&sample(1, 0)),
        encoded(&session(3, 0)),
        encoded(&wrong_version),
    ];
    for suffix in invalid {
        let bytes = [prefix.clone(), suffix].concat();
        fs::write(&path, &bytes).unwrap();
        let mut reader = Reader::new(&path);
        assert_eq!(reader.poll().unwrap(), [session(1, 0)]);
        assert!(reader.poll().is_err());
        assert!(reader.poll().is_err(), "invalid data stays at the cursor");
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn an_invalid_append_preserves_bytes_and_writer_sequence_state() {
    let directory = Directory::new();
    let path = directory.file();
    let mut writer = Writer::create(&path).unwrap();
    writer.append(&session(1, 0)).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(writer.append(&sample(2, 1)).is_err());
    assert!(writer.append(&session(1, 0)).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    writer.append(&sample(1, 1)).unwrap();
    assert_eq!(
        drain(&mut Reader::new(&path)),
        [session(1, 0), sample(1, 1)]
    );
}

#[test]
fn polls_bound_work_and_reject_truncation_of_committed_records() {
    let directory = Directory::new();
    let path = directory.file();
    let mut writer = Writer::create(&path).unwrap();
    writer.append(&session(1, 0)).unwrap();
    for tick in 1..=300 {
        writer.append(&sample(1, tick)).unwrap();
    }
    let mut reader = Reader::new(&path);
    assert_eq!(reader.poll().unwrap().len(), 128);
    assert_eq!(reader.poll().unwrap().len(), 128);
    assert_eq!(reader.poll().unwrap().len(), 45);
    assert!(reader.poll().unwrap().is_empty());
    let committed = fs::metadata(&path).unwrap().len();
    OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(committed - 1)
        .unwrap();
    assert!(reader.poll().is_err());
}

#[test]
fn oversized_records_fail_before_writing_and_cannot_be_misread_as_pending() {
    let directory = Directory::new();
    let path = directory.file();
    let mut writer = Writer::create(&path).unwrap();
    writer.append(&session(1, 0)).unwrap();
    let before = fs::read(&path).unwrap();
    let large = vec![0_u8; MAX_RECORD_BYTES as usize];
    assert!(
        writer
            .append_sample(1, 1, &Value::Bytes(large.clone()))
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    // A byte string exceeds the record bound, even if its payload is incomplete.
    append_raw(&path, &[0x5a]);
    append_raw(&path, &((MAX_RECORD_BYTES + 1) as u32).to_be_bytes());
    append_raw(&path, &large);
    let mut reader = Reader::new(&path);
    assert_eq!(reader.poll().unwrap(), [session(1, 0)]);
    assert!(reader.poll().is_err());
}
