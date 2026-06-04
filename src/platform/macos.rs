use anyhow::{anyhow, Result};
use core_foundation::base::TCFType;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use user_idle::UserIdle;

pub fn idle_seconds() -> Result<u64> {
    let idle = UserIdle::get_time().map_err(|e| anyhow!("UserIdle::get_time: {e:?}"))?;
    Ok(idle.as_seconds())
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMCopyAssertionsStatus(out: *mut CFDictionaryRef) -> i32;
}

pub fn media_active() -> Result<bool> {
    let mut raw: CFDictionaryRef = std::ptr::null();
    let rc = unsafe { IOPMCopyAssertionsStatus(&mut raw) };
    if rc != 0 {
        return Err(anyhow!("IOPMCopyAssertionsStatus rc={rc}"));
    }
    if raw.is_null() {
        return Ok(false);
    }
    let dict: CFDictionary<CFString, CFNumber> =
        unsafe { CFDictionary::wrap_under_create_rule(raw) };

    // PreventUserIdleDisplaySleep is held by anything that needs the screen
    // awake — video playback, full-screen apps, screen sharing, presentations.
    //
    // PreventUserIdleSystemSleep is intentionally NOT checked: bluetoothd holds
    // it whenever Bluetooth is on, powerd holds it whenever the display is on,
    // backups/downloads hold it too. Using it would pin activity to ON
    // permanently on most Macs. The trade-off is that pure audio playback
    // (Spotify, Music) won't keep the user marked active.
    if let Some(n) = dict.find(CFString::from_static_string("PreventUserIdleDisplaySleep")) {
        if n.to_i64().unwrap_or(0) > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    #[test]
    fn idle_seconds_returns_some_value() -> Result<()> {
        let s = idle_seconds()?;
        if s >= 86_400 {
            return Err(anyhow!("implausible idle: {s}"));
        }
        Ok(())
    }

    #[test]
    fn media_active_does_not_error() -> Result<()> {
        let _ = media_active()?;
        Ok(())
    }
}
