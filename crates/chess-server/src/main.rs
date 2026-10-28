//! `chess-server` binary: loads the configuration, sets up logging and serves the API.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use chess_reviewer::{ReviewConfig, ReviewService};
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

    let mut state = chess_server::AppState::new(&config);
    match review_service() {
        Ok(service) => state = state.with_review(Arc::new(service)),
        Err(err) => tracing::warn!("Chess.com reviews disabled: {err:#}"),
    }
    match nnue_api(&config) {
        Ok(api) => {
            tokio::spawn(api.clone().maintain());
            state = state.with_nnue(api);
        }
        Err(err) => tracing::warn!("NNUE lab API disabled: {err:#}"),
    }
    axum::serve(listener, chess_server::api::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")
}

/// The Chess.com reviews, configured by `config/review.toml` and the
/// `CHESS_REVIEW_*` variables (see `chess-reviewer`).
fn review_service() -> Result<ReviewService> {
    let config = ReviewConfig::load().context("invalid review configuration")?;
    tracing::info!(
        cache = %config.cache.display(),
        engine = ?config.analysis.engine,
        workers = config.workers(),
        "Chess.com reviews"
    );
    ReviewService::new(config).context("cannot open the review cache")
}

/// The NNUE lab (`/api/v1`): its database in `lab_dir`.
fn nnue_api(config: &ServerConfig) -> Result<chess_nnue_api::NnueApi> {
    let paths = config.lab_paths();
    let db = config.lab_dir.join("lab.sqlite");
    let lab = chess_jobs::Lab::open(&db, paths.clone())
        .with_context(|| format!("cannot open {}", db.display()))?;
    let api_config = config.api_config();
    tracing::info!(
        database = %db.display(),
        datasets = %paths.datasets.display(),
        models = %paths.models.display(),
        auth = ?config.api_auth,
        keys = match &api_config.auth {
            chess_nnue_api::Auth::Keys(keys) => keys.len(),
            chess_nnue_api::Auth::Disabled => 0,
        },
        workers = api_config.worker_token.is_some(),
        "NNUE lab API on /api/v1, documentation on /docs"
    );
    Ok(chess_nnue_api::NnueApi::new(Arc::new(lab), api_config))
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
