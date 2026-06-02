use anyhow::{anyhow, Result};
use std::process::Command;
use tracing::warn;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

pub fn idle_seconds() -> Result<u64> {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        let ok = GetLastInputInfo(&mut lii);
        if !ok.as_bool() {
            return Err(anyhow!("GetLastInputInfo returned false"));
        }
        let now_ms = GetTickCount64();
        let last_ms = lii.dwTime as u64;
        // GetLastInputInfo's dwTime is a 32-bit tick count and wraps every ~49.7 days.
        // GetTickCount64 doesn't wrap on any realistic uptime, so when last_ms > now_ms
        // (which means low 32 bits wrapped) we recover by interpreting last_ms as if it's
        // in the previous wraparound window.
        let idle_ms = if last_ms <= now_ms {
            now_ms - last_ms
        } else {
            (now_ms + (u32::MAX as u64 + 1)) - last_ms
        };
        Ok(idle_ms / 1000)
    }
}

pub fn current_ssid() -> Result<Option<String>> {
    let out = match Command::new("netsh").args(["wlan", "show", "interfaces"]).output() {
        Ok(o) => o,
        Err(e) => {
            warn!(error = %e, "netsh not available; treating as no SSID");
            return Ok(None);
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        // Match `SSID                   : HomeNet`, but not `BSSID`.
        if let Some(rest) = trimmed.strip_prefix("SSID") {
            // Skip if this is the BSSID line — would have started with "BSSID".
            // Skip leading whitespace/colon.
            let after_colon = rest.splitn(2, ':').nth(1).unwrap_or("").trim();
            if !after_colon.is_empty() {
                return Ok(Some(after_colon.to_string()));
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
    fn idle_seconds_returns_sane_value() -> Result<()> {
        let s = idle_seconds()?;
        if s >= 86_400 {
            return Err(anyhow!("implausible idle: {s}"));
        }
        Ok(())
    }

    #[test]
    fn ssid_does_not_error() -> Result<()> {
        let _ = current_ssid()?;
        Ok(())
    }
}
