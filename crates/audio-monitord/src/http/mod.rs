pub mod routes;
pub mod sse;

use std::sync::Arc;
use std::time::Instant;

use audio_monitor_core::Snapshot;
use axum::Router;
use tokio::sync::watch;
use tower_http::trace::TraceLayer;

use crate::pactl_source::AudioSource;

#[derive(Clone)]
pub struct AppState {
    pub started_at: Instant,
    pub snapshot_rx: watch::Receiver<Snapshot>,
    pub source: Arc<dyn AudioSource>,
    /// Sender used by mutating endpoints to push a fresh snapshot
    /// immediately after the change (so tray frontends see the new
    /// `active` sink without waiting a full sample interval).
    pub snapshot_tx: watch::Sender<Snapshot>,
    pub host: String,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", axum::routing::get(routes::healthz))
        .route("/v1/info", axum::routing::get(routes::info))
        .route("/v1/snapshot", axum::routing::get(routes::snapshot))
        .route("/v1/sinks", axum::routing::get(routes::sinks))
        .route(
            "/v1/sinks/default",
            axum::routing::get(routes::default_sink).post(routes::set_default_sink),
        )
        .route("/v1/stream", axum::routing::get(sse::stream))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
