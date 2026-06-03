use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

pub fn idle_seconds() -> Result<u64> {
    imp::idle_seconds()
}

/// True if any process is holding an "inhibit idle" power assertion — set by
/// video and audio playback. Used to treat a user watching a movie as active
/// even though they're not generating HID events.
pub fn media_active() -> Result<bool> {
    imp::media_active()
}
