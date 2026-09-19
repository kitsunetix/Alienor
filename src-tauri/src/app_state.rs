use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::app_config::AppConfig;
use crate::player::MpvPlayer;

pub(crate) struct AppState {
    pub(crate) player: Arc<MpvPlayer>,
    pub(crate) port: u16,
    pub(crate) last_seek: Arc<AtomicU64>,
    pub(crate) config: Arc<tokio::sync::Mutex<AppConfig>>,
}
