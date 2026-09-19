use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app_config::AppConfig;
use crate::player::{Error as PlayerError, MpvPlayer};
use serde_json::Value as JsonValue;

const STATUS_CACHE_MAX_AGE: Duration = Duration::from_millis(50);

pub(crate) struct StatusCache {
    pub(crate) value: Option<JsonValue>,
    pub(crate) updated_at: Option<Instant>,
}

impl Default for StatusCache {
    fn default() -> Self {
        Self {
            value: None,
            updated_at: None,
        }
    }
}

pub(crate) struct AppState {
    pub(crate) player: Arc<MpvPlayer>,
    pub(crate) port: u16,
    pub(crate) last_seek: Arc<AtomicU64>,
    pub(crate) config: Arc<tokio::sync::Mutex<AppConfig>>,
    pub(crate) status_cache: tokio::sync::Mutex<StatusCache>,
}

impl AppState {
    pub(crate) async fn get_cached_status(&self) -> Result<JsonValue, PlayerError> {
        let mut cache = self.status_cache.lock().await;

        if let (Some(value), Some(updated_at)) = (&cache.value, cache.updated_at) {
            if updated_at.elapsed() < STATUS_CACHE_MAX_AGE {
                return Ok(value.clone());
            }
        }

        let value = self.player.get_status()?;
        cache.value = Some(value.clone());
        cache.updated_at = Some(Instant::now());
        Ok(value)
    }
}
