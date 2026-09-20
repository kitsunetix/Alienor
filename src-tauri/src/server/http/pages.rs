use axum::{extract::State as AxumState, http::StatusCode, response::Html};
use once_cell::sync::Lazy;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;

use crate::app_state::AppState;

pub(crate) async fn status_page(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, String)> {
    static HTML_TEMPLATE: Lazy<String> =
        Lazy::new(|| include_str!("../../../templates/status.html").to_string());

    let current_port = state.port;
    let config = state.config.lock().await;
    let configured_port = config
        .port
        .map_or("Auto (Default: 3000)".to_string(), |port| port.to_string());
    drop(config);

    let html = HTML_TEMPLATE
        .replace("{current_port}", &current_port.to_string())
        .replace("{configured_port}", &configured_port)
        .replace("{port}", &current_port.to_string());

    Ok(Html(html))
}

pub(crate) fn find_templates_dir() -> PathBuf {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()));
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

    let mut candidates = vec![
        StdPath::new(MANIFEST_DIR).join("src-tauri/templates"),
        StdPath::new(MANIFEST_DIR).join("templates"),
        current_dir.join("src-tauri/templates"),
        current_dir.join("templates"),
    ];

    if let Some(directory) = executable_dir {
        candidates.push(directory.join("../Resources/templates"));
        candidates.push(directory.join("resources/templates"));
        candidates.push(directory.join("templates"));
    }

    for path in &candidates {
        if path.exists() {
            println!("Using templates directory: {:?}", path);
            return path.clone();
        }
    }

    let fallback = current_dir.join("src-tauri/templates");
    println!(
        "Templates directory not found in expected locations, falling back to {:?}",
        fallback
    );
    fallback
}
