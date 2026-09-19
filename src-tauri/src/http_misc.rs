use axum::{extract::Path, response::Json};
use serde_json::json;

#[tauri::command]
pub(crate) async fn sync_room(room_id: String) -> Result<String, String> {
    println!("Sync request for room: {}", room_id);
    Ok(format!("Connected to room {}", room_id))
}

pub(crate) async fn room_status(Path(room_id): Path<String>) -> Json<serde_json::Value> {
    Json(json!({
        "room": room_id,
        "status": "active",
        "users": []
    }))
}

pub(crate) async fn sync() -> Json<serde_json::Value> {
    Json(json!({
        "status": "synced",
        "timestamp": chrono::Utc::now().timestamp()
    }))
}
