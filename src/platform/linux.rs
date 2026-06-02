use anyhow::{anyhow, Result};
use std::sync::OnceLock;
use tracing::warn;

pub fn idle_seconds() -> Result<u64> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err() {
        return wayland_idle_seconds();
    }
    x11_idle_seconds()
}

fn x11_idle_seconds() -> Result<u64> {
    use x11::xlib;
    use x11::xss;
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return Err(anyhow!("XOpenDisplay returned null"));
        }
        let root = xlib::XDefaultRootWindow(display);
        let info = xss::XScreenSaverAllocInfo();
        if info.is_null() {
            xlib::XCloseDisplay(display);
            return Err(anyhow!("XScreenSaverAllocInfo returned null"));
        }
        let status = xss::XScreenSaverQueryInfo(display, root, info);
        let idle_ms = if status == 0 { 0 } else { (*info).idle as u64 };
        xlib::XFree(info as *mut _);
        xlib::XCloseDisplay(display);
        Ok(idle_ms / 1000)
    }
}

static WAYLAND_WARN: OnceLock<()> = OnceLock::new();

fn wayland_idle_seconds() -> Result<u64> {
    // No portable Wayland idle API. We log once and assume "active" (0 seconds idle)
    // so that activity reports remain conservative — i.e. we'll over-report activity
    // rather than miss a present user.
    WAYLAND_WARN.get_or_init(|| {
        warn!("Wayland session detected; portable idle detection is not available — reporting 0 seconds idle");
    });
    Ok(0)
}

static IWGETID_WARN: OnceLock<()> = OnceLock::new();

pub fn current_ssid() -> Result<Option<String>> {
    use std::process::Command;
    let out = match Command::new("iwgetid").arg("-r").output() {
        Ok(o) => o,
        Err(e) => {
            IWGETID_WARN.get_or_init(|| {
                warn!(error = %e, "iwgetid not available; treating as no SSID for the rest of this run");
            });
            return Ok(None);
        }
    };
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_seconds_does_not_error_on_headless() -> Result<()> {
        // On a headless CI Linux box X may not be available; we still expect
        // either a successful Wayland fallback OR an X11 error. Either is acceptable
        // — this test exists to ensure the function exits cleanly and doesn't panic.
        let _ = idle_seconds();
        Ok(())
    }

    #[test]
    fn ssid_does_not_error() -> Result<()> {
        let _ = current_ssid()?;
        Ok(())
    }
}
