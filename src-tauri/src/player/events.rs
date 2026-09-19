use libmpv2::events::Event;
use std::sync::atomic::Ordering;

use super::state::MpvPlayer;

impl MpvPlayer {
    pub fn check_events(&self) {
        if let Ok(mut handle) = self.handle.lock() {
            let mut saw_shutdown = false;
            let mut saw_end_file = false;
            let mut saw_file_loaded = false;

            {
                let event_context = handle.event_context_mut();
                while let Some(Ok(event)) = event_context.wait_event(0.01) {
                    match event {
                        Event::Shutdown => saw_shutdown = true,
                        Event::EndFile(_) => saw_end_file = true,
                        Event::FileLoaded => saw_file_loaded = true,
                        _ => {}
                    }
                }
            }

            if saw_shutdown {
                println!("MPV_EVENT_SHUTDOWN received");
                self.quit_flag.store(true, Ordering::Relaxed);
            }
            if saw_end_file {
                println!("MPV_EVENT_END_FILE received");
            }
            if saw_file_loaded {
                if let Err(e) = handle.set_property("pause", true) {
                    eprintln!("Failed to pause on file load: {}", e);
                }

                if let Some(time) = self.get_last_moon_time_seconds() {
                    let offset = self.get_offset_seconds();
                    let adjusted = (time + offset).max(0.0);
                    println!(
                        "MPV file loaded: seeking to Moon time {:.3}s (adjusted {:.3}s, offset {:.3}s)",
                        time, adjusted, offset
                    );
                    if let Err(e) =
                        handle.command("seek", &[&adjusted.to_string(), "absolute", "exact"])
                    {
                        eprintln!("Failed to seek on file load: {}", e);
                    }
                } else {
                    println!("MPV file loaded: no cached Moon time yet, skipping auto-seek.");
                }
            }
        } else {
            eprintln!("check_events: Failed to acquire MPV lock (potential poison/deadlock?)");
            self.quit_flag.store(true, Ordering::Relaxed);
        }
    }

    pub fn is_shutdown(&self) -> bool {
        if self.quit_flag.load(Ordering::Relaxed) {
            return true;
        }

        match self.handle.lock() {
            Ok(handle) => {
                let is_unresponsive = handle.get_property::<bool>("idle-active").is_err();
                if is_unresponsive {
                    println!("is_shutdown: Failed to get property after lock, assuming shutdown.");
                }
                is_unresponsive
            }
            Err(_) => {
                eprintln!(
                    "is_shutdown: Failed to acquire MPV lock (mutex poisoned?), assuming shutdown"
                );
                true
            }
        }
    }

    pub fn exit(&self) {
        self.quit_flag.store(true, Ordering::Relaxed);
        if let Ok(handle) = self.handle.try_lock() {
            let _ = handle.command("quit", &[]);
        } else {
            eprintln!(
                "exit: Could not acquire MPV lock to send quit command (lock held elsewhere?)"
            );
        }
    }
}
