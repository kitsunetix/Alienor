use axum::{extract::State as AxumState, http::StatusCode, response::Json};
use serde_json::json;
use std::sync::atomic::Ordering;
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

pub(crate) async fn set_playback_time(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    const MIN_SEEK_INTERVAL: u64 = 16;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let time = payload["time"].as_f64().ok_or((
        StatusCode::BAD_REQUEST,
        "Missing or invalid 'time' field".to_string(),
    ))?;
    state.player.set_last_moon_time_seconds(time);

    match state.player.get_handle() {
        Ok(handle) => {
            if handle.get_property::<String>("path").is_err() {
                println!(
                    "HTTP seek: Player is idle (no path property), ignoring seek request to {}.",
                    time
                );
                return Ok(Json(json!({
                    "status": "success",
                    "ignored": true,
                    "reason": "Player is idle (no media loaded)",
                    "time": time,
                    "timestamp": now
                })));
            }
        }
        Err(error) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get MPV handle: {}", error),
            ));
        }
    }

    if now - state.last_seek.load(Ordering::Relaxed) < MIN_SEEK_INTERVAL {
        return Ok(Json(json!({
            "status": "rate_limited",
            "message": "Too many seek requests"
        })));
    }

    let offset = state.player.get_offset_seconds();
    let adjusted_time = time + offset;
    println!(
        "HTTP seek: Attempting seek to {} (adjusted from {} with offset {})",
        adjusted_time, time, offset
    );

    match state
        .player
        .command("seek", &[&adjusted_time.to_string(), "absolute", "exact"])
    {
        Ok(_) => {
            state.last_seek.store(now, Ordering::Relaxed);
            Ok(Json(json!({
                "status": "success",
                "ignored": false,
                "time": time,
                "adjusted_time": adjusted_time,
                "offset": offset,
                "timestamp": now
            })))
        }
        Err(error) => {
            let error_string = error.to_string();
            eprintln!(
                "Error executing MPV seek command even after idle check: {}",
                error_string
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Seek command failed unexpectedly: {}", error_string),
            ))
        }
    }
}

pub(crate) async fn set_offset(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let offset_seconds = if let Some(seconds) = payload.get("seconds").and_then(|v| v.as_f64()) {
        state
            .player
            .set_offset_seconds(seconds)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        seconds
    } else if let Some(frames) = payload.get("frames").and_then(|v| v.as_i64()) {
        let fps = payload.get("fps").and_then(|v| v.as_f64()).unwrap_or(30.0);
        state
            .player
            .set_offset_frames(frames as i32, fps)
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        frames as f64 / fps
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing 'seconds' or 'frames' parameter".to_string(),
        ));
    };

    println!("Updated offset to {} seconds via HTTP", offset_seconds);
    Ok(Json(json!({
        "status": "success",
        "offset_seconds": offset_seconds
    })))
}

pub(crate) async fn get_offset(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(json!({
        "status": "success",
        "offset_seconds": state.player.get_offset_seconds()
    })))
}

pub(crate) async fn set_loop(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let enabled = payload.get("enabled").and_then(|value| value.as_bool()).ok_or((
        StatusCode::BAD_REQUEST,
        "Missing or invalid 'enabled' field".to_string(),
    ))?;

    state
        .player
        .set_loop(enabled)
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok(Json(json!({
        "status": "success",
        "loop_enabled": enabled
    })))
}

pub(crate) async fn get_loop(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let loop_enabled = state
        .player
        .get_loop()
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok(Json(json!({
        "status": "success",
        "loop_enabled": loop_enabled
    })))
}
