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

pub fn current_ssid() -> Result<Option<String>> {
    imp::current_ssid()
}
