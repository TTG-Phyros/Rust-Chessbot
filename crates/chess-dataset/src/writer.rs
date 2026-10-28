use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::{DatasetError, HEADER_SIZE, MAGIC, RECORD_SIZE, Record, VERSION};

/// The header of a dataset file.
pub(crate) fn header() -> [u8; HEADER_SIZE] {
    let mut header = [0_u8; HEADER_SIZE];
    header[0..4].copy_from_slice(MAGIC);
    header[4..6].copy_from_slice(&VERSION.to_le_bytes());
    header[6..8].copy_from_slice(&(RECORD_SIZE as u16).to_le_bytes());
    header
}

/// `<path>.tmp`: where a file is written before it is published.
pub(crate) fn temp_path(path: &Path) -> PathBuf {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    PathBuf::from(temp)
}

/// Writes a dataset file. Records go to `<path>.tmp`, renamed to `<path>`
/// by [`DatasetWriter::finish`], so readers never see a half-written file.
pub struct DatasetWriter {
    file: BufWriter<File>,
    temp: PathBuf,
    path: PathBuf,
    count: u64,
}

impl DatasetWriter {
    /// Starts a dataset file at `path` (creating its directory).
    ///
    /// # Errors
    ///
    /// When the file cannot be created.
    pub fn create(path: &Path) -> Result<Self, DatasetError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = temp_path(path);
        let mut file = BufWriter::with_capacity(1 << 20, File::create(&temp)?);
        file.write_all(&header())?;
        Ok(Self {
            file,
            temp,
            path: path.to_owned(),
            count: 0,
        })
    }

    /// Appends a record.
    ///
    /// # Errors
    ///
    /// When writing fails.
    pub fn write(&mut self, record: &Record) -> Result<(), DatasetError> {
        self.file.write_all(&record.to_bytes())?;
        self.count += 1;
        Ok(())
    }

    /// Records written so far.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Flushes and publishes the file; returns its path and record count.
    ///
    /// # Errors
    ///
    /// When flushing or renaming fails.
    pub fn finish(mut self) -> Result<(PathBuf, u64), DatasetError> {
        self.file.flush()?;
        drop(self.file);
        std::fs::rename(&self.temp, &self.path)?;
        Ok((self.path, self.count))
    }
}
