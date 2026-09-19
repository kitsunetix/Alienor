use libmpv2::Mpv;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

use super::error::Error;
use super::state::MpvPlayer;
use super::config;

#[allow(dead_code)]
impl MpvPlayer {
    pub fn new() -> Result<(Self, mpsc::Receiver<()>), Error> {
        let mpv = Mpv::new().map_err(|e| Error::InitError(e.to_string()))?;

        let (_exit_sender, exit_receiver) = mpsc::channel();
        let quit_flag = Arc::new(AtomicBool::new(false));
        config::configure(&mpv)?;

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
