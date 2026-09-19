use std::sync::atomic::Ordering;

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
}
