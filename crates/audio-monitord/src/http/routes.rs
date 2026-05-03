use audio_monitor_core::{Sink, Snapshot};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use super::AppState;
use crate::sampler;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub uptime_s: u64,
}

pub async fn healthz(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        uptime_s: state.started_at.elapsed().as_secs(),
    })
}

#[derive(Serialize)]
pub struct InfoResponse {
    pub backend_version: &'static str,
    pub api_version: &'static str,
    pub host: String,
    pub server_version: Option<String>,
    pub default_sink: Option<String>,
    pub sink_count: usize,
}

pub async fn info(State(state): State<AppState>) -> Json<InfoResponse> {
    let snap = state.snapshot_rx.borrow();
    Json(InfoResponse {
        backend_version: env!("CARGO_PKG_VERSION"),
        api_version: audio_monitor_core::API_VERSION,
        host: snap.host.clone(),
        server_version: snap.server_version.clone(),
        default_sink: snap.default_sink.clone(),
        sink_count: snap.sinks.len(),
    })
}

pub async fn snapshot(State(state): State<AppState>) -> Json<Snapshot> {
    Json(state.snapshot_rx.borrow().clone())
}

pub async fn sinks(State(state): State<AppState>) -> Json<Vec<Sink>> {
    Json(state.snapshot_rx.borrow().sinks.clone())
}

#[derive(Serialize)]
pub struct DefaultSinkResponse {
    pub default_sink: Option<String>,
}

pub async fn default_sink(State(state): State<AppState>) -> Json<DefaultSinkResponse> {
    Json(DefaultSinkResponse {
        default_sink: state.snapshot_rx.borrow().default_sink.clone(),
    })
}

#[derive(Deserialize)]
pub struct SetDefaultSinkRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn set_default_sink(
    State(state): State<AppState>,
    Json(req): Json<SetDefaultSinkRequest>,
) -> Result<Json<Snapshot>, (StatusCode, Json<ErrorResponse>)> {
    if req.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "name must not be empty".into(),
            }),
        ));
    }
    if let Err(err) = state.source.set_default_sink(&req.name).await {
        tracing::warn!(error = %err, sink = %req.name, "set-default-sink failed");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: err.to_string(),
            }),
        ));
    }
    // Push a fresh snapshot right away so SSE clients reflect the change
    // without waiting for the next sampler tick.
    let fresh = sampler::build_snapshot(&state.host, state.source.as_ref()).await;
    let _ = state.snapshot_tx.send(fresh.clone());
    Ok(Json(fresh))
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}
