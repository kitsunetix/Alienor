use libmpv2::Mpv;

use super::error::Error;

const ESSENTIAL_OPTIONS: &[(&str, &str)] = &[
    ("force-window", "yes"),
    ("input-default-bindings", "yes"),
    ("title", "Alienor - Sync videos with Moon Animator"),
    ("keep-open", "yes"),
    ("keep-open-pause", "yes"),
    ("hwdec", "no"),
    ("ontop", "yes"),
    ("osc", "yes"),
    ("osd-level", "2"),
    ("loop-file", "inf"),
    ("loop-playlist", "inf"),
    ("script-opts", "ytdl_hook-ytdl_path=yt-dlp"),
    ("ytdl", "yes"),
    ("ytdl-format", "bestvideo[height<=?1080]+bestaudio/best"),
    ("ytdl-raw-options", "no-check-certificate="),
];

pub(super) fn configure(mpv: &Mpv) -> Result<(), Error> {
    load_scripts(mpv);

    for &(key, value) in ESSENTIAL_OPTIONS {
        mpv.set_property(key, value).map_err(|_| {
            Error::PropertyError(format!("Failed to set property {} to {}", key, value), -1)
        })?;
    }

    Ok(())
}

fn load_scripts(mpv: &Mpv) {
    let scripts_dir = if cfg!(debug_assertions) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        std::path::Path::new(&manifest_dir).join("scripts")
    } else {
        let exe_dir = std::env::current_exe()
            .expect("Failed to get executable path")
            .parent()
            .expect("Failed to get executable directory")
            .to_path_buf();
        let scripts_dir = exe_dir.join("resources").join("scripts");
        println!("Release mode scripts path: {:?}", scripts_dir);
        scripts_dir
    };

    if !scripts_dir.exists() {
        println!("Warning: Scripts directory not found at {:?}", scripts_dir);
        return;
    }

    println!("Found scripts directory at {:?}", scripts_dir);
    let mut script_paths = Vec::new();
    match std::fs::read_dir(&scripts_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().ends_with(".lua") {
                    let script_path = entry.path();
                    if script_path.exists() {
                        println!("Found script: {:?}", script_path);
                        script_paths.push(script_path);
                    }
                }
            }
        }
        Err(error) => println!("Error reading scripts directory: {}", error),
    }

    if script_paths.is_empty() {
        println!("No .lua scripts found in {:?}", scripts_dir);
        return;
    }

    let script_list = script_paths
        .iter()
        .map(|path| path.to_str().unwrap())
        .collect::<Vec<_>>()
        .join(";");

    println!("Loading scripts: {}", script_list);
    match mpv.set_property("scripts", script_list.as_str()) {
        Ok(_) => println!("Successfully loaded all scripts"),
        Err(error) => {
            println!("Failed to load scripts: {}", error);
            if let Ok(error_message) = mpv.get_property::<String>("error-string") {
                println!("MPV error details: {}", error_message);
            }
        }
    }
}
