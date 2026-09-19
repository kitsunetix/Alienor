use libmpv2::Mpv;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::{Arc, Mutex};

/// Shared runtime state owned by the video player.
pub struct MpvPlayer {
    pub(super) handle: Arc<Mutex<Mpv>>,
    pub(super) quit_flag: Arc<AtomicBool>,
    pub(super) offset_seconds: Arc<AtomicI64>,
    pub(super) last_moon_time_ms: Arc<AtomicI64>,
}

// libmpv's handle is protected by the mutex for every access.
unsafe impl Send for MpvPlayer {}
unsafe impl Sync for MpvPlayer {}
