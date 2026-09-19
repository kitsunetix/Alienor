use axum::extract::ws::{Message, WebSocket};
use serde_json::json;
use std::sync::Arc;

use crate::app_state::AppState;
use crate::player::Error as PlayerError;
use super::commands::CommandRequest;
use super::protocol::send_error;

pub(crate) async fn handle_command(
    command: CommandRequest,
    socket: &mut WebSocket,
    state: &Arc<AppState>,
) {
    match command {
        CommandRequest::LoadUrl(url) => {
            println!("Loading URL: {}", url);
            if let Err(error) = state.player.load_file(&url) {
                eprintln!("Error loading URL: {}", error);
                send_error(socket, "loadURL", &error.to_string()).await;
            }
        }
        CommandRequest::Play => match state.player.set_property("pause", "false") {
            Ok(_) => println!("Play command executed successfully via WebSocket"),
            Err(error) => {
                eprintln!("Error setting pause=false: {}", error);
                send_error(socket, "play", &error.to_string()).await;
            }
        },
        CommandRequest::Pause => match state.player.set_property("pause", "true") {
            Ok(_) => println!("Pause command executed successfully via WebSocket"),
            Err(error) => {
                eprintln!("Error setting pause=true: {}", error);
                send_error(socket, "pause", &error.to_string()).await;
            }
        },
        CommandRequest::Seek(position) => {
            println!("Seeking to: {}", position);
            let offset = state.player.get_offset_seconds();
            let adjusted_time = position + offset;
            println!(
                "WebSocket seek: adjusted to {} from {} (offset {})",
                adjusted_time, position, offset
            );
            if let Err(error) = state.player.command(
                "seek",
                &[&adjusted_time.to_string(), "absolute", "exact"],
            ) {
                eprintln!("Error seeking: {}", error);
                send_error(socket, "seek", &error.to_string()).await;
            }
        }
        CommandRequest::SetOffset { seconds, frames, fps } => {
            let mut offset_result: Result<f64, PlayerError> = Err(
                PlayerError::CommandError(
                    "Invalid offset parameters".to_string(),
                    -1,
                ),
            );
            let mut update_type = "seconds";
            let mut original_value = 0.0;

            if let Some(seconds) = seconds {
                println!("Setting offset to {} seconds via WebSocket", seconds);
                offset_result = state.player.set_offset_seconds(seconds).map(|_| seconds);
                original_value = seconds;
            } else if let Some(frames) = frames {
                println!(
                    "Setting offset to {} frames at {} fps via WebSocket",
                    frames, fps
                );
                let seconds = frames as f64 / fps;
                offset_result = state
                    .player
                    .set_offset_frames(frames as i32, fps)
                    .map(|_| seconds);
                update_type = "frames";
                original_value = frames as f64;
            }

            match offset_result {
                Ok(calculated_seconds) => {
                    let response_json = if update_type == "seconds" {
                        json!({
                            "status": "success",
                            "command": "offsetUpdated",
                            "seconds": calculated_seconds
                        })
                    } else {
                        json!({
                            "status": "success",
                            "command": "offsetUpdated",
                            "frames": original_value as i64,
                            "seconds": calculated_seconds,
                            "fps": fps
                        })
                    };

                    if let Ok(response_string) = serde_json::to_string(&response_json) {
                        if socket.send(Message::Text(response_string)).await.is_err() {
                            eprintln!("Error sending offset confirmation");
                        }
                    } else {
                        eprintln!("Error serializing offset confirmation");
                    }
                }
                Err(error) => {
                    eprintln!("Error setting offset: {}", error);
                    send_error(socket, "setOffset", &error.to_string()).await;
                }
            }
        }
        CommandRequest::GetOffset => {
            let offset = state.player.get_offset_seconds();
            let response = json!({
                "status": "success",
                "command": "offsetStatus",
                "seconds": offset
            });
            if let Ok(response_string) = serde_json::to_string(&response) {
                if socket.send(Message::Text(response_string)).await.is_err() {
                    eprintln!("Error sending offset status");
                }
            } else {
                eprintln!("Error serializing offset status");
            }
        }
        CommandRequest::SetLoop(enabled) => {
            println!("Setting loop to: {}", enabled);
            match state.player.set_loop(enabled) {
                Ok(_) => {
                    let response = json!({
                        "status": "success",
                        "command": "loopUpdated",
                        "enabled": enabled
                    });
                    if let Ok(response_string) = serde_json::to_string(&response) {
                        if socket.send(Message::Text(response_string)).await.is_err() {
                            eprintln!("Error sending loop confirmation");
                        }
                    } else {
                        eprintln!("Error serializing loop confirmation");
                    }
                }
                Err(error) => {
                    eprintln!("Error setting loop: {}", error);
                    send_error(socket, "setLoop", &error.to_string()).await;
                }
            }
        }
        CommandRequest::GetLoop => match state.player.get_loop() {
            Ok(enabled) => {
                let response = json!({
                    "status": "success",
                    "command": "loopStatus",
                    "enabled": enabled
                });
                if let Ok(response_string) = serde_json::to_string(&response) {
                    if socket.send(Message::Text(response_string)).await.is_err() {
                        eprintln!("Error sending loop status");
                    }
                } else {
                    eprintln!("Error serializing loop status");
                }
            }
            Err(error) => {
                eprintln!("Error getting loop status: {}", error);
            }
        },
        CommandRequest::Invalid { command, message } => {
            send_error(socket, &command, message).await;
        }
        CommandRequest::Unknown(command) => {
            eprintln!("Received unknown WebSocket command: {}", command);
            send_error(socket, &command, "Unknown command").await;
        }
    }
}
