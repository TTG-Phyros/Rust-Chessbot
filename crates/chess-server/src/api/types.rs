//! Types exchanged with the web UI.
//!
//! Each type derives [`TS`]: `cargo test` (or `just gen-types`) writes its
//! TypeScript definition to `web/src/api/generated/`, so front and back can't drift.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utoipa::ToSchema;

/// Response of `GET /api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, ToSchema)]
#[ts(export)]
pub struct HealthResponse {
    /// Always `"ok"` when the server answers.
    pub status: String,
    /// Version of the server binary.
    pub version: String,
}

/// Message sent by the UI over `/ws`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ClientMessage {
    /// Round-trip check; answered with [`ServerMessage::Pong`].
    Ping {
        /// Echoed back in the reply.
        id: u32,
    },
}

/// Message sent by the server over `/ws`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ServerMessage {
    /// First message of every connection.
    Hello {
        /// Version of the server binary.
        version: String,
    },
    /// Reply to [`ClientMessage::Ping`].
    Pong {
        /// Identifier of the ping.
        id: u32,
    },
    /// The client sent something the server could not understand.
    Error {
        /// Human-readable reason.
        message: String,
    },
}
