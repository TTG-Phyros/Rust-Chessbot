//! Deleting datasets and models, and the model the engine plays with.

use rusqlite::{OptionalExtension, params};

use super::{dataset_in, model_in};
use crate::ids::{DatasetId, ModelId};
use crate::types::DatasetStatus;
use crate::{Lab, LabError};

/// Key of the active model in the `settings` table.
const ACTIVE_MODEL: &str = "active_model";

impl Lab {
    /// Deletes a dataset and its files.
    ///
    /// # Errors
    ///
    /// [`LabError::Conflict`] while it is being generated or a queued or
    /// running job trains on it.
    pub fn delete_dataset(&self, id: DatasetId) -> Result<(), LabError> {
        let connection = self.lock();
        let dataset = dataset_in(&connection, id)?;
        if dataset.status == DatasetStatus::Generating {
            return Err(LabError::Conflict(format!(
                "dataset {id} is being generated: stop its job first"
            )));
        }
        // Job specs are JSON: the dataset's id appears in those that use it.
        let user: Option<String> = connection
            .query_row(
                "SELECT id FROM jobs WHERE status IN ('queued', 'running', 'stopping')
                 AND kind = 'training' AND instr(spec, ?1) > 0 LIMIT 1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(job) = user {
            return Err(LabError::Conflict(format!(
                "dataset {id} is used by job {job}: wait for it or stop it first"
            )));
        }
        connection.execute("DELETE FROM datasets WHERE id = ?1", [id.to_string()])?;
        drop(connection);
        let dir = self.paths.dataset_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Deletes a model and its file.
    ///
    /// # Errors
    ///
    /// [`LabError::Conflict`] while it is the active model.
    pub fn delete_model(&self, id: ModelId) -> Result<(), LabError> {
        let connection = self.lock();
        model_in(&connection, id)?;
        if active_in(&connection)? == Some(id) {
            return Err(LabError::Conflict(format!(
                "model {id} is the active model: activate another one first"
            )));
        }
        connection.execute("DELETE FROM models WHERE id = ?1", [id.to_string()])?;
        drop(connection);
        let file = self.paths.model_file(id);
        if file.exists() {
            std::fs::remove_file(file)?;
        }
        Ok(())
    }

    /// Makes `id` the model the engine plays with (`None`: back to the
    /// server's configured network).
    ///
    /// # Errors
    ///
    /// [`LabError::NotFound`] for an unknown model.
    pub fn set_active_model(&self, id: Option<ModelId>) -> Result<(), LabError> {
        let connection = self.lock();
        match id {
            Some(id) => {
                model_in(&connection, id)?;
                connection.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    params![ACTIVE_MODEL, id.to_string()],
                )?;
            }
            None => {
                connection.execute("DELETE FROM settings WHERE key = ?1", [ACTIVE_MODEL])?;
            }
        }
        Ok(())
    }

    /// The model the engine plays with, if one was activated.
    ///
    /// # Errors
    ///
    /// When the database fails.
    pub fn active_model(&self) -> Result<Option<ModelId>, LabError> {
        active_in(&self.lock())
    }
}

fn active_in(connection: &rusqlite::Connection) -> Result<Option<ModelId>, LabError> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [ACTIVE_MODEL],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value.and_then(|text| text.parse().ok()))
}
