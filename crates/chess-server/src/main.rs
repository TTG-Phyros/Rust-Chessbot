//! `chess-server` binary: loads the configuration, sets up logging and serves the API.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use chess_server::config::{LogFormat, ServerConfig};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    let config = ServerConfig::load().context("invalid configuration")?;
    if std::env::args().any(|arg| arg == "--healthcheck") {
        return healthcheck(config.bind.port());
    }
    init_logging(config.log_format);
    // Searches run on blocking threads and recurse deeply: give them room.
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .context("cannot start the async runtime")?
        .block_on(serve(config))
}

async fn serve(config: ServerConfig) -> Result<()> {
    tracing::info!(?config, "starting chess-server");

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("cannot listen on {}", config.bind))?;
    tracing::info!(addr = %config.bind, "listening");

    axum::serve(
        listener,
        chess_server::api::router(chess_server::AppState::new(&config)),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server error")
}

/// Docker `HEALTHCHECK`: queries `/api/health` on localhost with std only, so
/// the runtime image needs neither curl nor wget.
fn healthcheck(port: u16) -> Result<()> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .with_context(|| format!("cannot connect to {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"GET /api/health HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    ensure!(
        response.starts_with("HTTP/1.0 200") || response.starts_with("HTTP/1.1 200"),
        "unhealthy: {}",
        response.lines().next().unwrap_or("empty response")
    );
    Ok(())
}

fn init_logging(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "warn,chess_server=info,tower_http=info".into());
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        LogFormat::Pretty => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

/// Resolves on Ctrl+C, or SIGTERM on Unix (sent by `docker stop`).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::error!(%err, "cannot listen for Ctrl+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => tracing::error!(%err, "cannot listen for SIGTERM"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutting down");
}
