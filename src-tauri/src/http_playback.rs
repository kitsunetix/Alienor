use axum::{extract::State as AxumState, http::StatusCode, response::Json};
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;

pub(crate) async fn get_playback_time(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let handle = state
        .player
        .get_handle()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let time = handle
        .get_property::<f64>("time-pos")
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    let duration = handle
        .get_property::<f64>("duration")
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok(Json(json!({
        "time": time,
        "duration": duration
    })))
}
