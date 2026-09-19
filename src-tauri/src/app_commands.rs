use std::sync::Arc;
use std::time::Duration;

use crate::app_config::save_config;
use crate::app_state::AppState;

#[tauri::command]
pub(crate) async fn exit_app(app_handle: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    println!("Exit requested.");
    state.player.exit();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        app_handle.exit(0);
    });
    Ok(())
}

#[tauri::command]
pub(crate) async fn set_port_and_restart(app_handle: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>, new_port: u16) -> Result<(), String> {
    if !(1025..=65535).contains(&new_port) {
        return Err("Invalid port number. Must be between 1025 and 65535.".to_string());
    }
    let mut config = state.config.lock().await;
    config.port = Some(new_port);
    save_config(&app_handle, &config).map_err(|error| error.to_string())?;
    drop(config);
    state.player.exit();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    app_handle.restart();
}

#[tauri::command]
pub(crate) async fn reset_port_and_restart(app_handle: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut config = state.config.lock().await;
    config.port = None;
    save_config(&app_handle, &config).map_err(|error| error.to_string())?;
    drop(config);
    state.player.exit();
    tokio::time::sleep(Duration::from_millis(1500)).await;
    app_handle.restart();
}

#[tauri::command]
pub(crate) async fn clear_saved_port(app_handle: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut config = state.config.lock().await;
    if config.port.is_none() {
        return Ok(());
    }
    config.port = None;
    save_config(&app_handle, &config).map_err(|error| error.to_string())?;
    Ok(())
}
