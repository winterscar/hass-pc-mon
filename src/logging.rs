use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Returned guards must be held for the lifetime of the process so the
/// background writer keeps flushing.
pub struct LogGuards {
    _file_guard: Option<WorkerGuard>,
}

pub fn init() -> Result<LogGuards> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr_layer = fmt::layer().with_writer(std::io::stderr).with_target(false);

    let log_path = match std::env::var("HASS_PC_MON_LOG") {
        Ok(s) if s.is_empty() => None,            // explicitly disabled
        Ok(s) => Some(PathBuf::from(s)),
        Err(_) => default_log_path(),
    };

    let file_guard = if let Some(path) = log_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating log dir {}", parent.display()))?;
        }
        let dir = path.parent().unwrap_or(std::path::Path::new("."));
        let name = path.file_name().unwrap_or(std::ffi::OsStr::new("hass-pc-mon.log"));
        let appender = tracing_appender::rolling::never(dir, name);
        let (non_blocking, guard) = tracing_appender::non_blocking(appender);
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(false);
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .init();
        None
    };

    Ok(LogGuards { _file_guard: file_guard })
}

fn default_log_path() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        let mut p = PathBuf::from(home);
        p.push("Library/Logs/hass-pc-mon.log");
        Some(p)
    }
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var_os("LOCALAPPDATA")?;
        let mut p = PathBuf::from(local);
        p.push("hass-pc-mon");
        p.push("hass-pc-mon.log");
        Some(p)
    }
    #[cfg(target_os = "linux")]
    {
        // Linux uses journald via systemd — stderr only.
        None
    }
}
