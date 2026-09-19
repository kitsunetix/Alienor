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
mod http_pages;
mod http_control;

use axum::{
    extract::{Path, State as AxumState},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use player::MpvPlayer;
use app_config::{load_config, save_config, AppConfig};
use app_state::AppState;
use http_misc::sync_room;
use portpicker::pick_unused_port;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::ErrorKind;
use std::path::PathBuf;
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
            let static_path = http_pages::find_templates_dir();
            println!("Axum will serve static files from: {:?}", static_path);

            let axum_app = Router::new()
                .route("/ws", get(ws_server::handler))
                .route("/control/:action", post(http_control::control_player))
                .route("/room/:id", get(http_misc::room_status))
                .route("/sync", post(http_misc::sync))
                .route("/playback/time", get(http_playback::get_playback_time).post(http_playback::set_playback_time))
                .route("/playback/offset", get(http_playback::get_offset).post(http_playback::set_offset))
                .route("/playback/loop", get(http_playback::get_loop).post(http_playback::set_loop))
                .route("/status", get(http_playback::get_connection_status))
                .route("/", get(http_pages::status_page))
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
