use std::time::Duration;

pub(crate) const PLAYING_STATUS_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const PAUSED_STATUS_INTERVAL: Duration = Duration::from_millis(300);
pub(crate) const PING_INTERVAL: Duration = Duration::from_secs(15);
pub(crate) const PING_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const MAX_CONSECUTIVE_ERRORS: u32 = 20;
pub(crate) const MIN_STATUS_INTERVAL: u64 = 8;
pub(crate) const ERROR_BACKOFF: Duration = Duration::from_millis(100);
