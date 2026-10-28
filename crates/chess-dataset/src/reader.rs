use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::{
    DatasetError, EXTENSION, HEADER_SIZE, MAGIC, RECORD_SIZE, Record, RecordError, VERSION,
};

/// Checks a dataset header.
pub(crate) fn check_header(header: &[u8; HEADER_SIZE]) -> Result<(), DatasetError> {
    if &header[0..4] != MAGIC {
        return Err(DatasetError::BadMagic);
    }
    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != VERSION {
        return Err(DatasetError::UnsupportedVersion(version));
    }
    let record_size = u16::from_le_bytes([header[6], header[7]]);
    if usize::from(record_size) != RECORD_SIZE {
        return Err(DatasetError::RecordSize(record_size));
    }
    Ok(())
}

/// Streams the records of a dataset file in chunks: the file is never
/// loaded entirely in memory.
pub struct DatasetReader {
    file: BufReader<File>,
    /// Records read so far.
    index: u64,
}

impl DatasetReader {
    /// Opens a dataset file and checks its header.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or is not a dataset of this version.
    pub fn open(path: &Path) -> Result<Self, DatasetError> {
        let mut file = BufReader::with_capacity(1 << 20, File::open(path)?);
        let mut header = [0_u8; HEADER_SIZE];
        file.read_exact(&mut header).map_err(|err| {
            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                DatasetError::NoHeader
            } else {
                err.into()
            }
        })?;
        check_header(&header)?;
        Ok(Self { file, index: 0 })
    }

    /// Reads up to `max` records; an empty vector means the end of the file.
    ///
    /// # Errors
    ///
    /// When reading fails or a record is corrupt.
    pub fn read_chunk(&mut self, max: usize) -> Result<Vec<Record>, DatasetError> {
        let mut records = Vec::with_capacity(max.min(1 << 20));
        let mut bytes = [0_u8; RECORD_SIZE];
        while records.len() < max {
            match self.file.read_exact(&mut bytes) {
                Ok(()) => {
                    let record =
                        Record::from_bytes(&bytes).map_err(|reason| corrupt(self.index, reason))?;
                    records.push(record);
                    self.index += 1;
                }
                Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(err) => return Err(err.into()),
            }
        }
        Ok(records)
    }
}

fn corrupt(index: u64, reason: RecordError) -> DatasetError {
    DatasetError::Corrupt { index, reason }
}

/// Number of records in a dataset file, from its size.
#[must_use]
pub fn record_count(path: &Path) -> u64 {
    std::fs::metadata(path)
        .map(|meta| meta.len().saturating_sub(HEADER_SIZE as u64) / RECORD_SIZE as u64)
        .unwrap_or(0)
}

/// Finished dataset files of `dir` (sorted), optionally restricted to the
/// chunks of one run (`<prefix>-<number>`: run `main` does not pick up the
/// files of run `main-2`).
#[must_use]
pub fn dataset_files(dir: &Path, prefix: Option<&str>) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == EXTENSION))
        .filter(|path| {
            prefix.is_none_or(|prefix| {
                path.file_stem()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_prefix(prefix)?.strip_prefix('-'))
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use chess_core::Position;

    use super::*;
    use crate::{DatasetWriter, Outcome, test_dir};

    #[test]
    fn write_then_stream() {
        let dir = test_dir("dataset-io");
        let path = dir.join("sample.bin");
        let mut writer = DatasetWriter::create(&path).unwrap();
        let mut pos = Position::startpos();
        let mut expected = Vec::new();
        for (i, uci) in ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"].iter().enumerate() {
            let record = Record::new(&pos, i as i16 * 10, Outcome::WhiteWins);
            writer.write(&record).unwrap();
            expected.push(record);
            let mv = pos.parse_uci_move(uci).unwrap();
            pos.make_move(mv);
        }
        assert!(!path.exists(), "visible before finish");
        let (published, count) = writer.finish().unwrap();
        assert_eq!((published, count), (path.clone(), 5));
        assert_eq!(record_count(&path), 5);
        assert_eq!(dataset_files(&dir, None), vec![path.clone()]);
        assert!(dataset_files(&dir, Some("other")).is_empty());

        let mut reader = DatasetReader::open(&path).unwrap();
        let mut read = reader.read_chunk(3).unwrap();
        assert_eq!(read.len(), 3);
        read.extend(reader.read_chunk(10).unwrap());
        assert_eq!(read, expected);
        assert!(reader.read_chunk(10).unwrap().is_empty());
    }

    #[test]
    fn files_of_a_run_exclude_runs_sharing_its_prefix() {
        let dir = test_dir("dataset-prefix");
        for name in [
            "main-0000",
            "main-0001",
            "main-2-0000",
            "mainline-0000",
            "main-x",
        ] {
            std::fs::write(dir.join(format!("{name}.{EXTENSION}")), b"").unwrap();
        }
        let names = |prefix| {
            dataset_files(&dir, Some(prefix))
                .iter()
                .filter_map(|p| Some(p.file_stem()?.to_str()?.to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(names("main"), ["main-0000", "main-0001"]);
        assert_eq!(names("main-2"), ["main-2-0000"]);
        assert_eq!(dataset_files(&dir, None).len(), 5);
    }

    #[test]
    fn rejects_foreign_files() {
        let dir = test_dir("dataset-bad");
        let path = dir.join("bad.bin");
        std::fs::write(&path, b"definitely not a dataset").unwrap();
        assert!(matches!(
            DatasetReader::open(&path),
            Err(DatasetError::BadMagic)
        ));
        std::fs::write(&path, b"CBDS").unwrap();
        assert!(matches!(
            DatasetReader::open(&path),
            Err(DatasetError::NoHeader)
        ));
    }
}
