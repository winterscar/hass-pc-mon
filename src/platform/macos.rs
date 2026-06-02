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
    let Some(iface) = wifi_interface()? else {
        return Ok(None);
    };
    let out = Command::new("/usr/sbin/networksetup")
        .args(["-getairportnetwork", &iface])
        .output()
        .context("running networksetup -getairportnetwork")?;
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

/// Find the Wi-Fi device's BSD name (e.g. `en0`, `en1`) by asking
/// `networksetup -listallhardwareports`. Returns `Ok(None)` when no Wi-Fi
/// hardware port is present on this Mac.
fn wifi_interface() -> Result<Option<String>> {
    let out = Command::new("/usr/sbin/networksetup")
        .arg("-listallhardwareports")
        .output()
        .context("running networksetup -listallhardwareports")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The output is paragraphs of:
    //   Hardware Port: Wi-Fi
    //   Device: en0
    //   Ethernet Address: ...
    let mut lines = stdout.lines();
    while let Some(line) = lines.next() {
        if line.trim() == "Hardware Port: Wi-Fi" {
            if let Some(dev_line) = lines.next() {
                if let Some(dev) = dev_line.trim().strip_prefix("Device:") {
                    let dev = dev.trim();
                    if !dev.is_empty() {
                        return Ok(Some(dev.to_string()));
                    }
                }
            }
        }
    }
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
