use libmpv2::Mpv;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use super::error::Error;
use super::state::MpvPlayer;

#[allow(dead_code)]
impl MpvPlayer {
    pub fn new() -> Result<(Self, mpsc::Receiver<()>), Error> {
        let mpv = Mpv::new().map_err(|e| Error::InitError(e.to_string()))?;

        // Create a channel for exit notification
        let (_exit_sender, exit_receiver) = mpsc::channel();
        let quit_flag = Arc::new(AtomicBool::new(false));

        // Set essential properties for window visibility and UI
        let essential_opts = [
            ("force-window", "yes"),
            ("input-default-bindings", "yes"),
            ("title", "Alienor - Sync videos with Moon Animator"),
            ("keep-open", "yes"),
            ("keep-open-pause", "yes"),
            ("hwdec", "no"),
            // UI and OSC settings
            ("ontop", "yes"),
            ("osc", "yes"),
            ("osd-level", "2"),
            // Loop settings
            ("loop-file", "inf"),     // Set infinite looping by default
            ("loop-playlist", "inf"), // Also loop playlists
            // ("window-progress-style", "bar"),
            // ("background", "#121212"),
            // ("force-window-colors", "yes"),
            // YouTube support
            ("script-opts", "ytdl_hook-ytdl_path=yt-dlp"),
            ("ytdl", "yes"),
            ("ytdl-format", "bestvideo[height<=?1080]+bestaudio/best"),
            ("ytdl-raw-options", "no-check-certificate="),
        ];

        // Get the scripts directory path
        let scripts_dir = if cfg!(debug_assertions) {
            // In debug mode, use the manifest directory
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            std::path::Path::new(&manifest_dir).join("scripts")
        } else {
            // In release mode, use the executable's directory
            let exe_dir = std::env::current_exe()
                .expect("Failed to get executable path")
                .parent()
                .expect("Failed to get executable directory")
                .to_path_buf();
            let scripts_dir = exe_dir.join("resources").join("scripts");
            println!("Release mode scripts path: {:?}", scripts_dir);
            scripts_dir
        };

        // Check if scripts directory exists
        if !scripts_dir.exists() {
            println!("Warning: Scripts directory not found at {:?}", scripts_dir);
        } else {
            println!("Found scripts directory at {:?}", scripts_dir);
            // Collect all Lua scripts
            let mut script_paths = Vec::new();
            match std::fs::read_dir(&scripts_dir) {
                Ok(entries) => {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            if entry.file_name().to_string_lossy().ends_with(".lua") {
                                let script_path = entry.path();
                                if script_path.exists() {
                                    println!("Found script: {:?}", script_path);
                                    script_paths.push(script_path);
                                }
                            }
                        }
                    }
                }
                Err(e) => println!("Error reading scripts directory: {}", e),
            }

            // If we found scripts, load them all at once
            if !script_paths.is_empty() {
                let script_list = script_paths
                    .iter()
                    .map(|p| p.to_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(";"); // Use semicolon for Windows

                println!("Loading scripts: {}", script_list);
                match mpv.set_property("scripts", script_list.as_str()) {
                    Ok(_) => println!("Successfully loaded all scripts"),
                    Err(e) => {
                        println!("Failed to load scripts: {}", e);
                        // Try to get more detailed error information
                        if let Ok(error_msg) = mpv.get_property::<String>("error-string") {
                            println!("MPV error details: {}", error_msg);
                        }
                    }
                }
            } else {
                println!("No .lua scripts found in {:?}", scripts_dir);
            }
        }

        for (key, value) in essential_opts {
            mpv.set_property(key, value).map_err(|_e| {
                Error::PropertyError(format!("Failed to set property {} to {}", key, value), -1)
            })?;
        }

        // Create a default offset value
        // Using zero offset by default - user can adjust as needed
        let offset_seconds = Arc::new(AtomicI64::new(0));
        println!("Started with zero playback offset, can be adjusted via UI");
        let last_moon_time_ms = Arc::new(AtomicI64::new(-1));

        Ok((
            MpvPlayer {
                handle: Arc::new(Mutex::new(mpv)),
                quit_flag,
                offset_seconds,
                last_moon_time_ms,
            },
            exit_receiver,
        ))
    }

    pub fn get_handle(&self) -> Result<std::sync::MutexGuard<'_, Mpv>, Error> {
        self.handle.lock().map_err(Error::from)
    }

    pub(crate) fn get_handle_internal(&self) -> std::sync::MutexGuard<'_, Mpv> {
        self.handle.lock().expect("Internal MPV handle lock failed")
    }

}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        // MPV will be dropped automatically
    }
}
