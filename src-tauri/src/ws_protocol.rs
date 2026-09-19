use axum::extract::ws::{Message, WebSocket};
use serde_json::json;

pub(crate) async fn send_error(socket: &mut WebSocket, command: &str, error_message: &str) {
    let error_json = json!({
        "status": "error",
        "command": command,
        "error": error_message
    });

    match serde_json::to_string(&error_json) {
        Ok(error_string) => {
            if socket.send(Message::Text(error_string)).await.is_err() {
                eprintln!("Failed to send WebSocket error message");
            }
        }
        Err(error) => eprintln!("Failed to serialize error message: {}", error),
    }
}
