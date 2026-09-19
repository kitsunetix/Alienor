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

pub(crate) async fn get_connection_status(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let status = state
        .player
        .get_status()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    let get_str = |key: &str| status.get(key).and_then(|value| value.as_str()).unwrap_or("");
    let get_f64 = |key: &str| status.get(key).and_then(|value| value.as_f64()).unwrap_or(0.0);

    let eof_reached = get_str("EndOfFile") == "yes";
    let is_idle = get_str("Idle") == "yes";
    let is_paused = get_str("Status") == "Paused";
    let duration = get_f64("Duration");
    let position = get_f64("Position");
    let is_playing = !is_paused && !eof_reached && !is_idle;

    Ok(Json(json!({
        "connected": true,
        "playing": is_playing,
        "port": state.port,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "eof": eof_reached,
        "idle": is_idle,
        "duration": duration,
        "position": position,
        "paused": is_paused,
        "offset": state.player.get_offset_seconds()
    })))
}
