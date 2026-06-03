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

    // PreventUserIdleDisplaySleep — held by video playback
    // PreventUserIdleSystemSleep  — held by audio playback
    // NoIdleSleepAssertion is intentionally excluded: backups and downloads
    // hold it too, which would falsely mark a sleeping machine as active.
    for k in ["PreventUserIdleDisplaySleep", "PreventUserIdleSystemSleep"] {
        if let Some(n) = dict.find(CFString::from_static_string(k)) {
            if n.to_i64().unwrap_or(0) > 0 {
                return Ok(true);
            }
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
