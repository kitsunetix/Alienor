use axum::{extract::Path, extract::State, http::StatusCode, response::Json};
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;

pub(crate) async fn control_player(
    State(state): State<Arc<AppState>>,
    Path(action): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let command_result = match action.as_str() {
        "play" => {
            let handle = state
                .player
                .get_handle()
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
            if handle.get_property::<bool>("pause").unwrap_or(false) {
                handle
                    .set_property("pause", false)
                    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
            }
            Ok(())
        }
        "pause" => {
            let handle = state
                .player
                .get_handle()
                .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
            if !handle.get_property::<bool>("pause").unwrap_or(false) {
                handle
                    .set_property("pause", true)
                    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
            }
            Ok(())
        }
        "stop" => state.player.command("stop", &[]),
        "volume_up" => state.player.command("add", &["volume", "5"]),
        "volume_down" => state.player.command("add", &["volume", "-5"]),
        "seek_forward" => state.player.command("seek", &["10"]),
        "seek_backward" => state.player.command("seek", &["-10"]),
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid action".to_string())),
    };

    command_result
        .map(|_| Json(json!({ "status": "success", "action": action })))
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}
