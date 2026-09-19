use std::sync::atomic::Ordering;

use crate::app_state::AppState;

pub(crate) enum MoonSeekResult {
    Ignored { timestamp: u64 },
    RateLimited,
    Applied {
        adjusted_time: f64,
        offset: f64,
        timestamp: u64,
    },
}

pub(crate) fn seek_to_moon_time(
    state: &AppState,
    time: f64,
    now: u64,
) -> Result<MoonSeekResult, String> {
    const MIN_SEEK_INTERVAL: u64 = 16;

    state.player.set_last_moon_time_seconds(time);

    match state.player.get_handle() {
        Ok(handle) => {
            if handle.get_property::<String>("path").is_err() {
                println!(
                    "HTTP seek: Player is idle (no path property), ignoring seek request to {}.",
                    time
                );
                return Ok(MoonSeekResult::Ignored { timestamp: now });
            }
        }
        Err(error) => return Err(format!("Failed to get MPV handle: {}", error)),
    }

    if now - state.last_seek.load(Ordering::Relaxed) < MIN_SEEK_INTERVAL {
        return Ok(MoonSeekResult::RateLimited);
    }

    let offset = state.player.get_offset_seconds();
    let adjusted_time = time + offset;
    println!(
        "HTTP seek: Attempting seek to {} (adjusted from {} with offset {})",
        adjusted_time, time, offset
    );

    state
        .player
        .command("seek", &[&adjusted_time.to_string(), "absolute", "exact"])
        .map_err(|error| format!("Seek command failed unexpectedly: {}", error))?;

    state.last_seek.store(now, Ordering::Relaxed);
    Ok(MoonSeekResult::Applied {
        adjusted_time,
        offset,
        timestamp: now,
    })
}
