use axum::extract::{State as AxumState, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::extract::ws::Message;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{interval, sleep, Instant, MissedTickBehavior};

use crate::app_state::AppState;
use crate::ws_commands::CommandRequest;
use crate::ws_config::{
    ERROR_BACKOFF, MAX_CONSECUTIVE_ERRORS, MIN_STATUS_INTERVAL, PAUSED_STATUS_INTERVAL,
    PING_INTERVAL, PING_TIMEOUT, PLAYING_STATUS_INTERVAL,
};
use crate::ws_handlers::handle_command;

const RELEVANT_STATUS_KEYS: &[&str] = &[
    "Status", "Position", "Duration", "Path", "Title", "Loop", "Offset", "EndOfFile", "Idle",
];

pub(crate) async fn handler(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: axum::extract::ws::WebSocket, state: Arc<AppState>) {
    let mut current_interval_duration = PLAYING_STATUS_INTERVAL;
    let mut status_interval = interval(current_interval_duration);
    status_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_known_pause_state = false;

    let mut ping_interval = interval(PING_INTERVAL);
    ping_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut last_pong = Instant::now();
    let mut consecutive_errors = 0;
    let mut last_status_update = 0_u64;
    let mut last_sent_status: Option<JsonValue> = None;
    let mut status_buffer = String::with_capacity(1024);

    'connection: loop {
        tokio::select! {
            biased;
            result = socket.recv() => {
                match result {
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = Instant::now();
                        consecutive_errors = 0;
                    }
                    Some(Ok(Message::Text(text))) => {
                        last_pong = Instant::now();
                        consecutive_errors = 0;
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(command) = CommandRequest::from_json(&json) {
                                handle_command(command, &mut socket, &state).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        eprintln!("Clean WebSocket close received");
                        break 'connection;
                    }
                    Some(Err(error)) => {
                        eprintln!("WebSocket error: {}", error);
                        consecutive_errors += 1;
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            eprintln!("Too many errors, closing WebSocket");
                            break 'connection;
                        }
                        sleep(ERROR_BACKOFF).await;
                    }
                    None => {
                        eprintln!("WebSocket closed by client.");
                        break 'connection;
                    }
                    _ => {}
                }
            }
            _ = status_interval.tick() => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                if now - last_status_update < MIN_STATUS_INTERVAL {
                    continue;
                }

                match state.get_cached_status().await {
                    Ok(current_status) => {
                        let should_send = last_sent_status.as_ref().map_or(true, |last_status| {
                            RELEVANT_STATUS_KEYS.iter().any(|key| {
                                current_status.get(key) != last_status.get(key)
                            })
                        });

                        if should_send {
                            status_buffer.clear();
                            if let Ok(status_string) = serde_json::to_string(&current_status) {
                                status_buffer.push_str(&status_string);
                                if socket.send(Message::Text(status_buffer.clone())).await.is_ok() {
                                    last_sent_status = Some(current_status.clone());
                                    last_status_update = now;
                                    consecutive_errors = 0;

                                    let is_paused = current_status
                                        .get("Status")
                                        .and_then(|value| value.as_str()) == Some("Paused");
                                    let desired_interval = if is_paused {
                                        PAUSED_STATUS_INTERVAL
                                    } else {
                                        PLAYING_STATUS_INTERVAL
                                    };
                                    if is_paused != last_known_pause_state
                                        || current_interval_duration != desired_interval
                                    {
                                        status_interval = interval(desired_interval);
                                        status_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
                                        current_interval_duration = desired_interval;
                                        last_known_pause_state = is_paused;
                                    }
                                } else {
                                    consecutive_errors += 1;
                                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                                        break 'connection;
                                    }
                                    sleep(ERROR_BACKOFF).await;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        eprintln!("Error getting player status: {}", error);
                        consecutive_errors += 1;
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            break 'connection;
                        }
                        sleep(ERROR_BACKOFF).await;
                    }
                }
            }
            _ = ping_interval.tick() => {
                if last_pong.elapsed() > PING_TIMEOUT {
                    eprintln!("WebSocket ping timeout, closing connection.");
                    break 'connection;
                }
                if socket.send(Message::Ping(vec![])).await.is_err() {
                    consecutive_errors += 1;
                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        break 'connection;
                    }
                    sleep(ERROR_BACKOFF).await;
                }
            }
        }
    }
    eprintln!("WebSocket connection ended.");
}
