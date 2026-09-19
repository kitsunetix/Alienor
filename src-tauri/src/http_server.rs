use axum::{routing::{get, post}, Router};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::app_state::AppState;
use crate::{http_control, http_misc, http_pages, http_playback, ws_server};

pub(crate) fn router(state: Arc<AppState>, static_path: PathBuf) -> Router {
    Router::new()
        .route("/ws", get(ws_server::handler))
        .route("/control/:action", post(http_control::control_player))
        .route("/room/:id", get(http_misc::room_status))
        .route("/sync", post(http_misc::sync))
        .route(
            "/playback/time",
            get(http_playback::get_playback_time).post(http_playback::set_playback_time),
        )
        .route(
            "/playback/offset",
            get(http_playback::get_offset).post(http_playback::set_offset),
        )
        .route(
            "/playback/loop",
            get(http_playback::get_loop).post(http_playback::set_loop),
        )
        .route("/status", get(http_playback::get_connection_status))
        .route("/", get(http_pages::status_page))
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
