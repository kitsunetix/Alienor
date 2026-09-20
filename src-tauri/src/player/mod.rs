//! Player domain.
//!
//! Public boundary for the video player and its mpv-backed implementation.

mod commands;
mod config;
mod error;
mod events;
mod mpv;
mod properties;
mod state;

pub use error::Error;
pub use state::MpvPlayer;
