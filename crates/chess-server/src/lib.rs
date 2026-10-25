//! Library half of `chess-server`, split from `main.rs` so integration tests
//! can build the router without binding a socket.

pub mod api;
pub mod config;
