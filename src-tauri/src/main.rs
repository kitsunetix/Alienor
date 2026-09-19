// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod player;
mod app_config;
mod app_state;
mod server;
mod server_config;
mod player_monitor;
mod synchronization;
mod app_commands;

use axum::extract::State as AxumState;
use player::MpvPlayer;
use app_config::load_config;
use app_state::AppState;
use server::http::misc::sync_room;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::ErrorKind;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WindowEvent,
};

#[tokio::main]
async fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            sync_room,
            app_commands::exit_app,
            app_commands::set_port_and_restart,
            app_commands::reset_port_and_restart,
            app_commands::clear_saved_port
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            let config = load_config(&app_handle);
            let initial_config = Arc::new(tokio::sync::Mutex::new(config.clone()));

            let port = server_config::select_port(config.port);
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
            let static_path = server::http::pages::find_templates_dir();
            println!("Axum will serve static files from: {:?}", static_path);

            let axum_app = server::router(app_state.clone(), static_path);
            tokio::spawn(server::serve(port, axum_app));

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

            player_monitor::spawn(app_handle.clone(), Arc::clone(&player));

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
