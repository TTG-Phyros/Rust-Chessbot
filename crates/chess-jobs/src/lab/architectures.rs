//! Architectures saved for reuse by training jobs.

use rusqlite::{Connection, Row, params};
use serde_json::Value;

use super::{get, list, now, parse_id, parse_json, to_json};
use crate::ids::ArchitectureId;
use crate::types::StoredArchitecture;
use crate::{Lab, LabError};

const COLUMNS: &str = "id, name, hash, architecture, created_at";

impl Lab {
    /// Saves an architecture after validating it.
    ///
    /// # Errors
    ///
    /// [`LabError::Invalid`] with every problem of the document.
    pub fn create_architecture(&self, document: &Value) -> Result<StoredArchitecture, LabError> {
        let architecture = chess_nnue_spec::from_value(document)
            .map_err(LabError::Invalid)?
            .normalized();
        let id = ArchitectureId::generate();
        let connection = self.lock();
        connection.execute(
            &format!("INSERT INTO architectures ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5)"),
            params![
                id.to_string(),
                architecture.metadata.name,
                architecture.hash().to_string(),
                to_json(&architecture)?,
                now()
            ],
        )?;
        architecture_in(&connection, id)
    }

    /// A saved architecture.
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`].
    pub fn architecture(&self, id: ArchitectureId) -> Result<StoredArchitecture, LabError> {
        architecture_in(&self.lock(), id)
    }

    /// Saved architectures, newest first, older than `after`.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn architectures(
        &self,
        after: Option<ArchitectureId>,
        limit: usize,
    ) -> Result<Vec<StoredArchitecture>, LabError> {
        list(
            &self.lock(),
            "architectures",
            COLUMNS,
            after.map(|id| id.to_string()),
            limit,
            from_row,
        )
    }

    /// Deletes a saved architecture (jobs and models keep their copy).
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`].
    pub fn delete_architecture(&self, id: ArchitectureId) -> Result<(), LabError> {
        let deleted = self
            .lock()
            .execute("DELETE FROM architectures WHERE id = ?1", [id.to_string()])?;
        if deleted == 0 {
            return Err(LabError::NotFound {
                what: "architecture",
                id: id.to_string(),
            });
        }
        Ok(())
    }
}

pub(super) fn architecture_in(
    connection: &Connection,
    id: ArchitectureId,
) -> Result<StoredArchitecture, LabError> {
    get(
        connection,
        "architectures",
        COLUMNS,
        "architecture",
        &id.to_string(),
        from_row,
    )
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<StoredArchitecture> {
    Ok(StoredArchitecture {
        id: parse_id(row, 0)?,
        name: row.get(1)?,
        hash: row.get(2)?,
        architecture: parse_json(row, 3)?,
        created_at: row.get(4)?,
    })
}
