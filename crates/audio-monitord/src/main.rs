mod config;
mod http;
mod pactl_source;
mod sampler;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use audio_monitor_core::Snapshot;
use clap::Parser;
use config::Config;
use pactl_source::{AudioSource, MockSource, PactlSource};
use sampler::{build_snapshot, empty_snapshot};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::parse();
    init_tracing(&cfg.log_level);

    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string());

    let source: Arc<dyn AudioSource> = if cfg.mock {
        tracing::warn!("running with MOCK audio source");
        Arc::new(MockSource::new())
    } else {
        Arc::new(
            PactlSource::init()
                .await
                .context("failed to initialise pactl source; is pulseaudio/pipewire running?")?,
        )
    };

    let initial: Snapshot = match source.sample().await {
        Ok(_) => build_snapshot(&host, source.as_ref()).await,
        Err(err) => {
            tracing::warn!(error = %err, "initial sample failed; serving empty snapshot");
            empty_snapshot(&host, source.server_version())
        }
    };
    let (tx, rx) = watch::channel(initial);

    sampler::spawn(
        source.clone(),
        host.clone(),
        cfg.sample_interval_ms,
        tx.clone(),
    );

    let state = http::AppState {
        started_at: Instant::now(),
        snapshot_rx: rx,
        source,
        snapshot_tx: tx,
        host,
    };
    let app = http::build_router(state);

    let addr = SocketAddr::new(cfg.bind, cfg.port);
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!(%addr, "audio-monitord listening");

    tokio::select! {
        result = axum::serve(listener, app) => {
            result.context("HTTP server error")?;
        }
        _ = shutdown_signal() => {
            tracing::info!("shutdown requested; aborting in-flight SSE streams");
        }
    }

    tracing::info!("shutdown complete");
    Ok(())
}

fn init_tracing(directive: &str) {
    let filter = EnvFilter::try_new(directive).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.ok();
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                tracing::warn!(error = %err, "could not install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("ctrl-c received"),
        _ = terminate => tracing::info!("SIGTERM received"),
    }
}
