use axum::{routing::{get, post}, Router};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::app_state::AppState;

pub(crate) mod http {
    pub(crate) mod control;
    pub(crate) mod misc;
    pub(crate) mod pages;
    pub(crate) mod playback;
}

pub(crate) mod websocket {
    pub(crate) mod commands;
    pub(crate) mod config;
    pub(crate) mod handlers;
    pub(crate) mod protocol;
    pub(crate) mod server;
}

pub(crate) fn router(state: Arc<AppState>, static_path: PathBuf) -> Router {
    Router::new()
        .route("/ws", get(websocket::server::handler))
        .route("/control/:action", post(http::control::control_player))
        .route("/room/:id", get(http::misc::room_status))
        .route("/sync", post(http::misc::sync))
        .route(
            "/playback/time",
            get(http::playback::get_playback_time).post(http::playback::set_playback_time),
        )
        .route(
            "/playback/offset",
            get(http::playback::get_offset).post(http::playback::set_offset),
        )
        .route(
            "/playback/loop",
            get(http::playback::get_loop).post(http::playback::set_loop),
        )
        .route("/status", get(http::playback::get_connection_status))
        .route("/", get(http::pages::status_page))
        .nest_service("/static", ServeDir::new(static_path))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

pub(crate) async fn serve(port: u16, app: Router) {
    match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
        Ok(listener) => {
            println!("Server running on http://localhost:{}", port);
            if let Err(error) = axum::serve(listener, app).await {
                eprintln!("Server error: {}", error);
            }
        }
        Err(error) => {
            eprintln!("Failed to bind Axum server to port {}: {}", port, error);
        }
    }
}
