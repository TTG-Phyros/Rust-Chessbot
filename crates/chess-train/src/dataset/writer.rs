use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{HEADER_SIZE, MAGIC, RECORD_SIZE, Record, VERSION};

/// Writes a dataset file. Records go to `<path>.tmp`, renamed to `<path>`
/// by [`DatasetWriter::finish`], so readers never see a half-written file.
pub(crate) struct DatasetWriter {
    file: BufWriter<File>,
    temp: PathBuf,
    path: PathBuf,
    count: u64,
}

impl DatasetWriter {
    pub(crate) fn create(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        let temp = path.with_extension("tmp");
        let file =
            File::create(&temp).with_context(|| format!("cannot create {}", temp.display()))?;
        let mut file = BufWriter::with_capacity(1 << 20, file);
        let mut header = [0_u8; HEADER_SIZE];
        header[0..4].copy_from_slice(MAGIC);
        header[4..6].copy_from_slice(&VERSION.to_le_bytes());
        header[6..8].copy_from_slice(&(RECORD_SIZE as u16).to_le_bytes());
        file.write_all(&header)?;
        Ok(Self {
            file,
            temp,
            path: path.to_owned(),
            count: 0,
        })
    }

    pub(crate) fn write(&mut self, record: &Record) -> Result<()> {
        self.file.write_all(&record.to_bytes())?;
        self.count += 1;
        Ok(())
    }

    pub(crate) fn count(&self) -> u64 {
        self.count
    }

    /// Flushes and publishes the file; returns its path and record count.
    pub(crate) fn finish(mut self) -> Result<(PathBuf, u64)> {
        self.file.flush()?;
        drop(self.file);
        std::fs::rename(&self.temp, &self.path).with_context(|| {
            format!(
                "cannot rename {} to {}",
                self.temp.display(),
                self.path.display()
            )
        })?;
        Ok((self.path, self.count))
    }
}
