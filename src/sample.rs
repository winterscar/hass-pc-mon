use crate::platform;
use anyhow::Result;
use display_info::DisplayInfo;
use serde::Serialize;
use tracing::debug;

#[derive(Debug, Serialize, Clone)]
pub struct Sample {
    pub activity: bool,
    pub monitor_count: usize,
    pub monitor_names: Vec<String>,
}

pub fn take(idle_threshold_secs: u64) -> Result<Sample> {
    let idle = platform::idle_seconds()?;
    let input_active = idle < idle_threshold_secs;
    let media = platform::media_active().unwrap_or(false);
    let activity = input_active || media;
    if !input_active && media {
        debug!(idle, "no input but media assertion held — reporting active");
    }
    let monitors = DisplayInfo::all().unwrap_or_default();
    let monitor_names: Vec<String> = monitors
        .iter()
        // Prefer friendly_name: on macOS (and Windows) `name` is a synthetic
        // "Display {id}" while friendly_name carries the actual monitor name.
        .map(|d| {
            if !d.friendly_name.is_empty() {
                d.friendly_name.clone()
            } else if !d.name.is_empty() {
                d.name.clone()
            } else {
                format!("display-{}", d.id)
            }
        })
        .collect();
    Ok(Sample {
        activity,
        monitor_count: monitor_names.len(),
        monitor_names,
    })
}
