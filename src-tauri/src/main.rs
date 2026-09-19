// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod player;
mod app_config;
mod app_state;
mod ws_protocol;
mod ws_commands;
mod ws_handlers;
mod ws_config;
mod ws_server;
mod http_misc;
mod http_playback;

use axum::{
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use player::MpvPlayer;
use app_config::{load_config, save_config, AppConfig};
use app_state::AppState;
use http_misc::sync_room;
use once_cell::sync::Lazy;
use portpicker::pick_unused_port;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::ErrorKind;
use std::path::{Path as StdPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WindowEvent,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;


#[tauri::command]
async fn exit_app(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    println!("Exit requested.");
    state.player.exit();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        app_handle.exit(0);
    });
    Ok(())
}

#[tauri::command]
async fn set_port_and_restart(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    new_port: u16,
) -> Result<(), String> {
    println!("Set port and restart requested: New port = {}", new_port);
    if !(1025..=65535).contains(&new_port) {
        return Err("Invalid port number. Must be between 1025 and 65535.".to_string());
    }
    let mut config_guard = state.config.lock().await;
    config_guard.port = Some(new_port);
    if let Err(e) = save_config(&app_handle, &config_guard) {
        eprintln!("Failed to save config: {}", e);
        return Err(format!("Failed to save configuration: {}", e));
    }
    drop(config_guard);
    println!("Configuration saved. Attempting to restart application...");
    state.player.exit();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Call restart. This terminates the process if successful.
    app_handle.restart();
    // No code needed here. If restart() succeeds, process terminates.
    // If it somehow failed and returned, the function would implicitly complete,
    // satisfying the Result<(), String> signature, but this path isn't expected.
}

#[tauri::command]
async fn reset_port_and_restart(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    println!("Reset port to auto-detect and restart requested.");
    let mut config_guard = state.config.lock().await;
    config_guard.port = None;
    if let Err(e) = save_config(&app_handle, &config_guard) {
        eprintln!("Failed to save config for reset: {}", e);
        return Err(format!("Failed to save configuration for reset: {}", e));
    }
    drop(config_guard);
    println!("Configuration saved for reset. Attempting to restart application...");
    state.player.exit();
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Call restart. This terminates the process if successful.
    app_handle.restart();
    // No code needed here.
}

// --- New Command: Clear Saved Port (No Restart) ---
#[tauri::command]
async fn clear_saved_port(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    println!("Clear saved port requested.");
    let mut config_guard = state.config.lock().await;
    if config_guard.port.is_none() {
        println!("No specific port was saved. Nothing to clear.");
        return Ok(()); // Nothing to do
    }
    config_guard.port = None; // Set to None for auto-detect on next launch
    if let Err(e) = save_config(&app_handle, &config_guard) {
        eprintln!("Failed to save config for port clear: {}", e);
        return Err(format!(
            "Failed to save configuration for port clear: {}",
            e
        ));
    }
    println!("Saved port configuration cleared. Will use auto-detect on next launch.");
    Ok(())
}

async fn control_player(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Path(action): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cmd_result = match action.as_str() {
        "play" => {
            // First check if player is paused
            let handle = state
                .player
                .get_handle()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let is_paused = handle.get_property::<bool>("pause").unwrap_or(false);

            if is_paused {
                // Use direct set_property with bool value for better compatibility
                handle
                    .set_property("pause", false)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                Ok(())
            } else {
                // Already playing, just return success
                Ok(())
            }
        }
        "pause" => {
            // First check if player is playing
            let handle = state
                .player
                .get_handle()
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            let is_paused = handle.get_property::<bool>("pause").unwrap_or(false);

            if !is_paused {
                // Use direct set_property with bool value for better compatibility
                handle
                    .set_property("pause", true)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                Ok(())
            } else {
                // Already paused, just return success
                Ok(())
            }
        }
        "stop" => state.player.command("stop", &[]),
        "volume_up" => state.player.command("add", &["volume", "5"]),
        "volume_down" => state.player.command("add", &["volume", "-5"]),
        "seek_forward" => state.player.command("seek", &["10"]),
        "seek_backward" => state.player.command("seek", &["-10"]),
        _ => return Err((StatusCode::BAD_REQUEST, "Invalid action".to_string())),
    };

    cmd_result
        .map(|_| Json(json!({ "status": "success", "action": action })))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn status_page(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, String)> {
    // Cache the HTML template
    static HTML_TEMPLATE: Lazy<String> =
        Lazy::new(|| include_str!("../templates/status.html").to_string());

    // Get the current port from the state
    let current_port = state.port;
    // Get the configured port (if any) for display
    let config = state.config.lock().await;
    let configured_port_str = config
        .port
        .map_or("Auto (Default: 3000)".to_string(), |p| p.to_string());
    drop(config);

    // Replace placeholders with actual values
    let html = HTML_TEMPLATE
        .replace("{current_port}", &current_port.to_string())
        .replace("{configured_port}", &configured_port_str)
        // Keep replacing {port} for backward compatibility - remove semicolon
        .replace("{port}", &current_port.to_string());

    Ok(Html(html))
}

// Add new API endpoints for Roblox sync
async fn set_playback_time(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    const MIN_SEEK_INTERVAL: u64 = 16;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Get time from payload (store for file-load auto-seek)
    let time = payload["time"].as_f64().ok_or((
        StatusCode::BAD_REQUEST,
        "Missing or invalid 'time' field".to_string(),
    ))?;
    state.player.set_last_moon_time_seconds(time);

    // --- Explicit Idle Check First ---
    match state.player.get_handle() {
        Ok(handle) => {
            // Check if a path is loaded. If get_property fails, assume idle.
            if handle.get_property::<String>("path").is_err() {
                println!(
                    "HTTP seek: Player is idle (no path property), ignoring seek request to {}.",
                    time
                );
                return Ok(Json(json!({
                            "status": "success",
                            "ignored": true,
                            "reason": "Player is idle (no media loaded)",
                            "time": time, // Include requested time in response
                            "timestamp": now
                })));
            }
            // If path exists, proceed with the seek logic
        }
        Err(e) => {
            // Failed to get handle, this is a more significant error
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get MPV handle: {}", e),
            ));
        }
    }
    // --- End Idle Check ---

    // Rate limit check (only apply if not idle)
    if now - state.last_seek.load(Ordering::Relaxed) < MIN_SEEK_INTERVAL {
        return Ok(Json(
            json!({ "status": "rate_limited", "message": "Too many seek requests" }),
        ));
    }

    // Get offset
    let offset = state.player.get_offset_seconds();
    let adjusted_time = time + offset;

    // Attempt seek command (should succeed if we passed the idle check)
    println!(
        "HTTP seek: Attempting seek to {} (adjusted from {} with offset {})",
        adjusted_time, time, offset
    );
    match state
        .player
        .command("seek", &[&adjusted_time.to_string(), "absolute", "exact"])
    {
        Ok(_) => {
            // Seek succeeded
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
        Err(e) => {
            // If seek fails even after the idle check, it's an unexpected error
            let error_string = e.to_string();
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

// Add these handlers for offset functionality
async fn set_offset(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Removed file reload and delayed commands

    // Handle both seconds and frames offsets
    let offset_seconds = if let Some(seconds) = payload.get("seconds").and_then(|v| v.as_f64()) {
        state
            .player
            .set_offset_seconds(seconds)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        seconds
    } else if let Some(frames) = payload.get("frames").and_then(|v| v.as_i64()) {
        // Default to 30 fps if not specified
        let fps = payload.get("fps").and_then(|v| v.as_f64()).unwrap_or(30.0);

        state
            .player
            .set_offset_frames(frames as i32, fps)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        (frames as f64) / fps
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing 'seconds' or 'frames' parameter".to_string(),
        ));
    };

    // Just updating the internal offset is sufficient now.
    // The change will be reflected in subsequent get_status and seek calls.
    println!("Updated offset to {} seconds via HTTP", offset_seconds);

    Ok(Json(json!({
        "status": "success",
        "offset_seconds": offset_seconds
    })))
}

async fn get_offset(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let offset_seconds = state.player.get_offset_seconds();

    Ok(Json(json!({
        "status": "success",
        "offset_seconds": offset_seconds
    })))
}

// New endpoints for loop control
async fn set_loop(
    AxumState(state): AxumState<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Get loop setting from payload
    let enabled = payload.get("enabled").and_then(|v| v.as_bool()).ok_or((
        StatusCode::BAD_REQUEST,
        "Missing or invalid 'enabled' field".to_string(),
    ))?;

    state
        .player
        .set_loop(enabled)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "status": "success",
        "loop_enabled": enabled
    })))
}

async fn get_loop(
    AxumState(state): AxumState<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let loop_enabled = state
        .player
        .get_loop()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "status": "success",
        "loop_enabled": loop_enabled
    })))
}

// --- Start: Restore find_templates_dir helper ---
fn find_templates_dir() -> PathBuf {
    // Executable directory
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    // Current working directory
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // Cargo manifest dir (compile‑time)
    const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

    // Build list of candidate paths
    let mut candidates: Vec<PathBuf> = Vec::new();
    // dev paths
    candidates.push(StdPath::new(MANIFEST_DIR).join("src-tauri/templates"));
    candidates.push(StdPath::new(MANIFEST_DIR).join("templates"));
    candidates.push(cwd.join("src-tauri/templates"));
    candidates.push(cwd.join("templates"));
    // production paths (resources next to exe, or inside Resources on macOS)
    if let Some(ref dir) = exe_dir {
        candidates.push(dir.join("../Resources/templates")); // macOS bundle structure
        candidates.push(dir.join("resources/templates")); // Other platforms
        candidates.push(dir.join("templates")); // Fallback if directly next to exe
    }

    // First path that exists wins
    for p in &candidates {
        if p.exists() {
            println!("Using templates directory: {:?}", p);
            return p.clone();
        }
    }

    // Fallback if nothing found
    let fallback = cwd.join("src-tauri/templates"); // Should ideally not be reached in prod
    println!(
        "Templates directory not found in expected locations, falling back to {:?}",
        fallback
    );
    fallback
}
// --- End: Restore find_templates_dir helper ---

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            sync_room,
            exit_app,
            set_port_and_restart,
            reset_port_and_restart,
            clear_saved_port
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            let config = load_config(&app_handle);
            let initial_config = Arc::new(tokio::sync::Mutex::new(config.clone()));

            // --- Determine Port (Sync check) ---
            let port = match config.port {
                Some(p) if (1025..=65535).contains(&p) => {
                    match std::net::TcpListener::bind(format!("0.0.0.0:{}", p)) {
                        Ok(listener) => { drop(listener); p },
                        Err(e) => {
                            eprintln!("Configured port {} is unavailable: {}. Falling back.", p, e);
                            pick_unused_port().expect("No ports available")
                        }
                    }
                },
                Some(p) => {
                    eprintln!("Configured port {} is invalid. Falling back.", p);
                    pick_unused_port().expect("No ports available")
                }
                None => {
                    match std::net::TcpListener::bind(("0.0.0.0", 3000)) {
                        Ok(listener) => { drop(listener); 3000 },
                        Err(_) => pick_unused_port().expect("No ports available"),
                    }
                }
            };
            let url = format!("http://localhost:{}", port);

            // --- Initialize MPV Player ---
            let (player, _exit_receiver) = MpvPlayer::new().expect("Failed to initialize MPV player");
            let player = Arc::new(player);

            // --- Create App State ---
            let app_state = Arc::new(AppState {
                player: Arc::clone(&player),
                port,
                last_seek: Arc::new(AtomicU64::new(0)),
                config: initial_config,
                status_cache: tokio::sync::Mutex::new(app_state::StatusCache::default()),
            });
            app.manage(app_state.clone());

            // --- Start Axum Server ---
            let static_path = find_templates_dir();
            println!("Axum will serve static files from: {:?}", static_path);

            let axum_app = Router::new()
                .route("/ws", get(ws_server::handler))
                .route("/control/:action", post(control_player))
                .route("/room/:id", get(http_misc::room_status))
                .route("/sync", post(http_misc::sync))
                .route("/playback/time", get(http_playback::get_playback_time).post(set_playback_time))
                .route("/playback/offset", get(get_offset).post(set_offset))
                .route("/playback/loop", get(get_loop).post(set_loop))
                .route("/status", get(http_playback::get_connection_status))
                .route("/", get(status_page))
                .nest_service("/static", ServeDir::new(static_path))
                .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
                .with_state(app_state.clone());

            tokio::spawn(async move {
                match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
                    Ok(listener) => {
                        println!("Server running on http://localhost:{}", port);
                        if let Err(e) = axum::serve(listener, axum_app).await {
                            eprintln!("Server error: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to bind Axum server to port {}: {}", port, e);
                    }
                }
            });

            // --- Create Tray Menu (Using Builder Pattern) ---
            let quit_item = MenuItemBuilder::new("Quit Alienor")
                .id("quit")
                .build(&app_handle)?;
            let show_item = MenuItemBuilder::new("Show Main Window")
                .id("show_main")
                .build(&app_handle)?;

            let tray_menu = MenuBuilder::new(&app_handle)
                .items(&[&show_item, &quit_item])
                .build()?;

            // --- Tray Icon Setup (Handling events inline) ---
            let url_clone_for_menu = url.clone(); // Clone url for menu event closure
            let url_clone_for_tray = url.clone(); // Clone url for tray event closure (Corrected: Separate clone)

            let _tray_icon = TrayIconBuilder::new()
                .menu(&tray_menu)
                .tooltip("Alienor")
                .icon(app_handle.default_window_icon().cloned().ok_or_else(|| tauri::Error::InvalidIcon(std::io::Error::new(ErrorKind::NotFound, "Default icon not found")))?) 
                .on_menu_event(move |app, event| {
                    let url_clone = url_clone_for_menu.clone(); // Clone again for move closure
                    match event.id.as_ref() {
                        "quit" => {
                            println!("Quit requested from tray.");
                            app.exit(0);
                        }
                        "show_main" => { // Always create new window
                            println!("Show main window requested from tray menu. Creating new window...");
                            if let Err(e) = tauri::WebviewWindowBuilder::new(
                                app,
                                "main", 
                                WebviewUrl::External(url_clone.parse().expect("Invalid external URL in menu handler")),
                            )
                            .title("Alienor")
                            .inner_size(800.0, 600.0)
                            .build()
                            {
                                eprintln!("Failed to create main window from tray menu: {}", e);
                            }
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    let url_clone = url_clone_for_tray.clone(); // Clone again for move closure
                    match event {
                        TrayIconEvent::Click { button: MouseButton::Left, .. } => { // Always create new window
                             println!("Tray icon clicked. Creating new main window...");
                             let app = tray.app_handle();
                             if let Err(e) = tauri::WebviewWindowBuilder::new(
                                app,
                                "main",
                                WebviewUrl::External(url_clone.parse().expect("Invalid external URL in tray handler")),
                            )
                            .title("Alienor")
                            .inner_size(800.0, 600.0)
                            .build()
                            {
                                eprintln!("Failed to create main window from tray icon click: {}", e);
                            }
                        }
                        _ => {} // Ignore other tray events
                    }
                })
                .build(&app_handle)?;

             // --- Create Dummy Background Window --- 
             let _dummy_window = tauri::WebviewWindowBuilder::new(
                app,
                "background-runner", // Unique label
                 WebviewUrl::App("".into()), // Load empty content
             )
             .visible(false) // Keep it hidden
             .skip_taskbar(true) // Don't show in taskbar
             .title("Alienor Background Runner") // Optional title
             .inner_size(1.0, 1.0) // Minimal size
             .build()?;

            // --- Register Updater Plugin (Correct Way) ---
            app.handle().plugin(tauri_plugin_updater::Builder::default().build())?;

            // --- Create Initial Main Window (Using External URL) ---
            let _main_window = match app.get_webview_window("main") {
                Some(win) => win, // Should ideally not exist yet, but handle defensively
                None => {
                    tauri::WebviewWindowBuilder::new(
                        app,
                        "main",
                        WebviewUrl::External(url.parse().expect("Invalid external URL for initial window")),
                    )
                    .title("Alienor")
                    .inner_size(800.0, 600.0)
                    .build()?
                }
            };

            // --- Monitor MPV Events ---
            let app_handle_clone = app_handle.clone(); // Clone AppHandle for the monitor thread
            let player_clone = Arc::clone(&player);
            std::thread::spawn(move || {
                loop {
                    player_clone.check_events();
                    if player_clone.is_shutdown() {
                        println!("MPV player shutdown detected by monitor thread. Exiting application.");
                        app_handle_clone.exit(0); // Exit the entire application
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => { // Removed `api`
                if window.label() == "main" { // Only handle main window close
                    println!("Main window close requested. Allowing close (webview terminates).");
                    // No hide, no prevent_close. Let it close naturally.
                } else if window.label() == "background-runner" {
                    println!("Background runner window close requested (should not happen normally). Ignoring.");
                    // Optionally prevent close here if needed, but usually unnecessary
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
