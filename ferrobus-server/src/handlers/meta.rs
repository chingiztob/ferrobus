use axum::{Json, extract::State};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct HealthzResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct MetaResponse {
    server_version: &'static str,
    stop_count: usize,
    route_count: usize,
    feeds_info: String,
}

pub(crate) async fn healthz() -> Json<HealthzResponse> {
    Json(HealthzResponse { status: "ok" })
}

pub(crate) async fn meta(State(state): State<AppState>) -> Json<MetaResponse> {
    Json(MetaResponse {
        server_version: env!("CARGO_PKG_VERSION"),
        stop_count: state.model.stop_count(),
        route_count: state.model.route_count(),
        feeds_info: state.model.feeds_info(),
    })
}
