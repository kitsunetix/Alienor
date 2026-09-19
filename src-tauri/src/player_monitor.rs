use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;

use crate::player::MpvPlayer;

pub(crate) fn spawn(app_handle: AppHandle, player: Arc<MpvPlayer>) {
    std::thread::spawn(move || loop {
        player.check_events();
        if player.is_shutdown() {
            println!("MPV player shutdown detected by monitor thread. Exiting application.");
            app_handle.exit(0);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    });
}
