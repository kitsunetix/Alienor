//! Player domain.
//!
//! This module is the public boundary for video playback.  The implementation
//! is still backed by the legacy `mpv` module for now so the migration can be
//! done incrementally without changing runtime behaviour.

mod commands;
mod config;
mod error;
mod events;
mod mpv;
mod properties;
mod state;

#[path = "../mpv.rs"]
mod legacy_mpv;

pub use error::Error;
pub use state::MpvPlayer;
