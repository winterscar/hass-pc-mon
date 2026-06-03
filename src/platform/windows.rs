use anyhow::{anyhow, Result};
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

pub fn media_active() -> Result<bool> {
    // Windows equivalent would be SetThreadExecutionState/PowerGetActiveScheme
    // inspection; not implemented yet. Always return false.
    Ok(false)
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
}
