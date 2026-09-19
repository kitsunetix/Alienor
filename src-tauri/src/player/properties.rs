use std::sync::atomic::Ordering;
use serde_json::{self, json, Value as JsonValue};

use super::error::Error;
use super::state::MpvPlayer;

impl MpvPlayer {
    /// Stores the playback offset in milliseconds for precise synchronization.
    pub fn set_offset_seconds(&self, offset: f64) -> Result<(), Error> {
        let millis = (offset * 1000.0) as i64;
        self.offset_seconds.store(millis, Ordering::Relaxed);
        println!("Stored playback offset: {} seconds ({} ms)", offset, millis);
        Ok(())
    }

    /// Stores a frame offset after converting it to seconds using the animation FPS.
    pub fn set_offset_frames(&self, frames: i32, fps: f64) -> Result<(), Error> {
        let seconds = frames as f64 / fps;
        let millis = (seconds * 1000.0) as i64;
        self.offset_seconds.store(millis, Ordering::Relaxed);
        println!(
            "Stored playback offset: {} frames ({} seconds at {} fps, {} ms)",
            frames, seconds, fps, millis
        );
        Ok(())
    }

    pub fn get_offset_seconds(&self) -> f64 {
        self.offset_seconds.load(Ordering::Relaxed) as f64 / 1000.0
    }

    pub fn set_last_moon_time_seconds(&self, time: f64) {
        if !time.is_finite() {
            return;
        }
        let clamped = if time < 0.0 { 0.0 } else { time };
        let millis = (clamped * 1000.0).round() as i64;
        self.last_moon_time_ms.store(millis, Ordering::Relaxed);
        println!("Cached Moon time: {:.3}s ({} ms)", clamped, millis);
    }

    pub fn get_last_moon_time_seconds(&self) -> Option<f64> {
        let millis = self.last_moon_time_ms.load(Ordering::Relaxed);
        if millis < 0 {
            None
        } else {
            Some(millis as f64 / 1000.0)
        }
    }

    pub fn get_status(&self) -> Result<JsonValue, Error> {
        let handle = self.handle.lock()?;

        let time_pos_opt = handle.get_property::<f64>("time-pos").ok();
        let duration_opt = handle.get_property::<f64>("duration").ok();
        let path_opt = handle.get_property::<String>("path").ok();
        let volume_opt = handle.get_property::<f64>("volume").ok();
        let speed_opt = handle.get_property::<f64>("speed").ok();
        let loop_file_opt = handle.get_property::<String>("loop-file").ok();
        let pause_opt = handle.get_property::<bool>("pause").ok();
        let eof_reached_opt = handle.get_property::<String>("eof-reached").ok();
        let idle_active_opt = handle.get_property::<bool>("idle-active").ok();
        let media_title_opt = handle.get_property::<String>("media-title").ok();

        let fps_opt = handle
            .get_property::<f64>("container-fps")
            .or_else(|_| handle.get_property::<f64>("estimated-vf-fps"))
            .ok();

        let is_idle = idle_active_opt.unwrap_or(path_opt.is_none());
        let is_paused = pause_opt.unwrap_or(is_idle);
        let status_str = if is_idle {
            "Idle".to_string()
        } else if is_paused {
            "Paused".to_string()
        } else {
            "Playing".to_string()
        };

        let current_offset = self.get_offset_seconds();
        let adjusted_time_pos = time_pos_opt.map(|time| time - current_offset);

        Ok(json!({
            "Status": status_str,
            "Position": adjusted_time_pos,
            "Elapsed": adjusted_time_pos,
            "Duration": duration_opt,
            "Path": path_opt,
            "Title": media_title_opt.or(path_opt),
            "Volume": volume_opt.map(|volume| volume.round()),
            "Speed": speed_opt,
            "Loop": loop_file_opt.map_or(false, |loop_state| loop_state == "inf" || loop_state == "yes"),
            "Offset": current_offset,
            "EndOfFile": eof_reached_opt,
            "Idle": is_idle,
            "fps": fps_opt,
        }))
    }
}
