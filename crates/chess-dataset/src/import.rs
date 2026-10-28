//! Validated import of a dataset received as a stream (an upload), and the
//! content hash and statistics of existing files.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::reader::check_header;
use crate::stats::DatasetStats;
use crate::writer::temp_path;
use crate::{DatasetError, DatasetReader, HEADER_SIZE, RECORD_SIZE, Record};

/// A dataset checked record by record.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// Records.
    pub records: u64,
    /// Size of the files, headers included.
    pub bytes: u64,
    /// Content hash (SHA-256 of the records, hex).
    pub sha256: String,
    /// Statistics.
    pub stats: DatasetStats,
}

/// Content hash and statistics, fed one record at a time.
#[derive(Debug, Clone, Default)]
pub struct ContentHasher {
    sha: Sha256,
    stats: DatasetStats,
}

impl ContentHasher {
    /// Counts `record`.
    pub fn add(&mut self, record: &Record) {
        // Records are hashed as written by this crate (reserved bytes zero),
        // so the hash does not depend on how a file was produced.
        self.sha.update(record.to_bytes());
        self.stats.add(record);
    }

    /// The result, for files totalling `bytes`.
    #[must_use]
    pub fn finish(self, bytes: u64) -> Imported {
        Imported {
            records: self.stats.records,
            bytes,
            sha256: hex(&self.sha.finalize()),
            stats: self.stats,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Reads a dataset from `source` (at most `max_bytes`), checks its header
/// and every record, and writes it to `dest` through a temporary file:
/// `dest` appears only once the whole dataset is valid.
///
/// # Errors
///
/// [`DatasetError`] naming the first problem: header, corrupt or truncated
/// record, size limit, no record, or an I/O error. Nothing is left on disk.
pub fn import(source: impl Read, dest: &Path, max_bytes: u64) -> Result<Imported, DatasetError> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = temp_path(dest);
    let result = copy_checked(source, &temp, max_bytes);
    match result {
        Ok(imported) => {
            std::fs::rename(&temp, dest)?;
            Ok(imported)
        }
        Err(err) => {
            let _ = std::fs::remove_file(&temp);
            Err(err)
        }
    }
}

fn copy_checked(
    mut source: impl Read,
    temp: &Path,
    max_bytes: u64,
) -> Result<Imported, DatasetError> {
    let mut header = [0_u8; HEADER_SIZE];
    source.read_exact(&mut header).map_err(|err| {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            DatasetError::NoHeader
        } else {
            err.into()
        }
    })?;
    check_header(&header)?;

    let mut out = BufWriter::with_capacity(1 << 20, File::create(temp)?);
    out.write_all(&header)?;
    let mut hasher = ContentHasher::default();
    let mut total = HEADER_SIZE as u64;
    let mut buffer = vec![0_u8; 1 << 16];
    // Bytes of a record split across two reads.
    let mut pending = 0;
    loop {
        let read = match source.read(&mut buffer[pending..]) {
            Ok(0) => break,
            Ok(read) => read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        };
        total += read as u64;
        if total > max_bytes {
            return Err(DatasetError::TooLarge(max_bytes));
        }
        let available = pending + read;
        let (records, rest) = buffer[..available].as_chunks::<RECORD_SIZE>();
        for bytes in records {
            let record = Record::from_bytes(bytes).map_err(|reason| DatasetError::Corrupt {
                index: hasher.stats.records,
                reason,
            })?;
            out.write_all(&record.to_bytes())?;
            hasher.add(&record);
        }
        pending = rest.len();
        let consumed = available - pending;
        buffer.copy_within(consumed..available, 0);
    }
    if pending > 0 {
        return Err(DatasetError::Truncated(hasher.stats.records));
    }
    if hasher.stats.records == 0 {
        return Err(DatasetError::Empty);
    }
    out.flush()?;
    out.into_inner()
        .map_err(std::io::IntoInnerError::into_error)?
        .sync_all()?;
    Ok(hasher.finish(total))
}

/// Content hash and statistics of the dataset made of `files`, in order.
///
/// # Errors
///
/// When a file cannot be read or holds a corrupt record.
pub fn scan(files: &[PathBuf]) -> Result<Imported, DatasetError> {
    let mut hasher = ContentHasher::default();
    let mut bytes = 0;
    for path in files {
        bytes += std::fs::metadata(path)?.len();
        let mut reader = DatasetReader::open(path)?;
        loop {
            let chunk = reader.read_chunk(1 << 16)?;
            if chunk.is_empty() {
                break;
            }
            for record in &chunk {
                hasher.add(record);
            }
        }
    }
    Ok(hasher.finish(bytes))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use chess_core::Position;

    use super::*;
    use crate::writer::header;
    use crate::{DatasetWriter, Outcome, test_dir};

    fn records() -> Vec<Record> {
        let endgame = Position::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        (0..100)
            .map(|i| {
                let pos = if i % 2 == 0 {
                    Position::startpos()
                } else {
                    endgame.clone()
                };
                Record::new(&pos, i * 7, Outcome::Draw)
            })
            .collect()
    }

    fn file_bytes(records: &[Record]) -> Vec<u8> {
        let mut bytes = header().to_vec();
        for record in records {
            bytes.extend_from_slice(&record.to_bytes());
        }
        bytes
    }

    /// A reader returning at most 7 bytes per call: records span reads.
    struct Trickle<'a>(&'a [u8]);

    impl Read for Trickle<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(7).min(self.0.len());
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn imports_valid_streams_and_hashes_like_a_scan() {
        let dir = test_dir("import");
        let records = records();
        let bytes = file_bytes(&records);
        let dest = dir.join("uploaded.bin");
        let imported = import(Trickle(&bytes), &dest, 1 << 20).unwrap();
        assert_eq!(imported.records, 100);
        assert_eq!(imported.bytes, bytes.len() as u64);
        assert_eq!(std::fs::read(&dest).unwrap(), bytes);
        assert_eq!(imported.stats.pieces[3], 50);

        // The same records split in two files hash the same.
        let parts = [dir.join("a.bin"), dir.join("b.bin")];
        for (part, records) in parts.iter().zip(records.chunks(60)) {
            let mut writer = DatasetWriter::create(part).unwrap();
            for record in records {
                writer.write(record).unwrap();
            }
            writer.finish().unwrap();
        }
        let scanned = scan(&parts).unwrap();
        assert_eq!(scanned.sha256, imported.sha256);
        assert_eq!(scanned.stats, imported.stats);
        assert_eq!(scanned.bytes, imported.bytes + 16);
        assert_eq!(imported.sha256.len(), 64);
    }

    #[test]
    fn rejects_bad_streams_without_leaving_files() {
        let dir = test_dir("import-bad");
        let dest = dir.join("bad.bin");
        let good = file_bytes(&records());
        let check = |bytes: &[u8], limit: u64| {
            let err = import(bytes, &dest, limit).unwrap_err();
            assert!(!dest.exists());
            assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "{err}");
            err
        };

        assert!(matches!(check(b"CBD", 1 << 20), DatasetError::NoHeader));
        assert!(matches!(
            check(b"PK\x03\x04 not a dataset at all", 1 << 20),
            DatasetError::BadMagic
        ));
        let mut version = good.clone();
        version[4] = 9;
        assert!(matches!(
            check(&version, 1 << 20),
            DatasetError::UnsupportedVersion(9)
        ));
        assert!(matches!(
            check(&good[..good.len() - 5], 1 << 20),
            DatasetError::Truncated(99)
        ));
        let mut corrupt = good.clone();
        corrupt[HEADER_SIZE + 3 * RECORD_SIZE + 26] = 9;
        assert!(matches!(
            check(&corrupt, 1 << 20),
            DatasetError::Corrupt { index: 3, .. }
        ));
        assert!(matches!(check(&good, 1000), DatasetError::TooLarge(1000)));
        assert!(matches!(check(&header(), 1 << 20), DatasetError::Empty));
    }
}
