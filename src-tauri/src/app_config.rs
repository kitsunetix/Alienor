use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

const CONFIG_FILE_NAME: &str = "alienor_config.json";
const LEGACY_CONFIG_FILE_NAME: &str = "alien_config.json";

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub(crate) struct AppConfig {
    pub(crate) port: Option<u16>,
}

fn get_config_path(app_handle: &AppHandle) -> std::io::Result<PathBuf> {
    let config_dir = app_handle.path().app_config_dir().map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("App config directory not found: {}", error),
        )
    })?;
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join(CONFIG_FILE_NAME))
}

fn get_legacy_config_path(app_handle: &AppHandle) -> std::io::Result<PathBuf> {
    let config_dir = app_handle.path().app_config_dir().map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("App config directory not found: {}", error),
        )
    })?;
    Ok(config_dir.join(LEGACY_CONFIG_FILE_NAME))
}

pub(crate) fn load_config(app_handle: &AppHandle) -> AppConfig {
    match get_config_path(app_handle) {
        Ok(path) => {
            let default_path = path.clone();
            let legacy_path = get_legacy_config_path(app_handle).ok();
            let load_path = if path.exists() {
                path
            } else if let Some(legacy) = legacy_path {
                if legacy.exists() {
                    println!("Using legacy config path: {:?}", legacy);
                    legacy
                } else {
                    default_path.clone()
                }
            } else {
                default_path.clone()
            };

            if load_path.exists() {
                match File::open(&load_path) {
                    Ok(mut file) => {
                        let mut contents = String::new();
                        if file.read_to_string(&mut contents).is_ok() {
                            match serde_json::from_str(&contents) {
                                Ok(config) => {
                                    println!("Loaded config from {:?}: {:?}", load_path, config);
                                    config
                                }
                                Err(error) => {
                                    eprintln!(
                                        "Failed to parse config file {:?}: {}. Using default.",
                                        load_path, error
                                    );
                                    AppConfig::default()
                                }
                            }
                        } else {
                            eprintln!("Failed to read config file {:?}. Using default.", load_path);
                            AppConfig::default()
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "Failed to open config file {:?}: {}. Using default.",
                            load_path, error
                        );
                        AppConfig::default()
                    }
                }
            } else {
                println!("Config file {:?} not found. Using default.", default_path);
                AppConfig::default()
            }
        }
        Err(error) => {
            eprintln!("Failed to determine config path: {}. Using default.", error);
            AppConfig::default()
        }
    }
}

pub(crate) fn save_config(app_handle: &AppHandle, config: &AppConfig) -> std::io::Result<()> {
    let path = get_config_path(app_handle)?;
    let contents = serde_json::to_string_pretty(config)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let mut file = File::create(&path)?;
    file.write_all(contents.as_bytes())?;
    println!("Saved config to {:?}: {:?}", path, config);
    Ok(())
}
