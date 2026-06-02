use anyhow::{anyhow, Context, Result};
use core_graphics::event::CGEventType;
use core_graphics::event_source::CGEventSourceStateID;
use std::process::Command;

// `CGEventSource::seconds_since_last_event_type` is NOT provided by the
// core-graphics 0.23 crate, so we bind the underlying C function directly.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(
        stateID: CGEventSourceStateID,
        eventType: CGEventType,
    ) -> f64;
}

pub fn idle_seconds() -> Result<u64> {
    // CGEventType::Null (== 0 == kCGAnyInputEventType) reports across any input event.
    let secs = unsafe {
        CGEventSourceSecondsSinceLastEventType(
            CGEventSourceStateID::HIDSystemState,
            CGEventType::Null,
        )
    };
    if !secs.is_finite() || secs < 0.0 {
        return Err(anyhow!("CGEventSource returned non-finite seconds: {secs}"));
    }
    Ok(secs as u64)
}

pub fn current_ssid() -> Result<Option<String>> {
    let out = Command::new("/usr/sbin/networksetup")
        .args(["-getairportnetwork", "en0"])
        .output()
        .context("running networksetup")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Expected format: "Current Wi-Fi Network: HomeNet"
    if let Some(rest) = stdout.split_once("Current Wi-Fi Network:") {
        let ssid = rest.1.trim();
        if ssid.is_empty() || ssid.to_lowercase().contains("not associated") {
            return Ok(None);
        }
        return Ok(Some(ssid.to_string()));
    }
    // "You are not associated with an AirPort network." style output.
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn idle_seconds_returns_some_value() -> Result<()> {
        let s = idle_seconds()?;
        // Sanity: on a fresh test run idle should be well under a day.
        if s >= 86_400 {
            return Err(anyhow!("implausible idle: {s}"));
        }
        Ok(())
    }

    #[test]
    fn ssid_does_not_error() -> Result<()> {
        // May return Some or None depending on host. Either is fine.
        let _ = current_ssid()?;
        Ok(())
    }
}
