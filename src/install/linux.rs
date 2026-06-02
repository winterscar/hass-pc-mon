use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

const UNIT_NAME: &str = "hass-pc-mon.service";

fn unit_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set")?;
    let mut p = PathBuf::from(home);
    p.push(".config/systemd/user");
    std::fs::create_dir_all(&p).context("creating systemd user dir")?;
    p.push(UNIT_NAME);
    Ok(p)
}

pub fn install() -> Result<()> {
    let exe = std::env::current_exe().context("resolving current exe")?;
    let unit = unit_path()?;
    let exe_s = exe.display().to_string();
    let contents = format!(r#"[Unit]
Description=hass-pc-mon — report PC state to MQTT

[Service]
ExecStart={exe_s} run
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#);
    std::fs::write(&unit, contents).with_context(|| format!("writing {}", unit.display()))?;
    info!(path = %unit.display(), "wrote systemd user unit");

    let status = Command::new("systemctl").args(["--user", "daemon-reload"]).status()
        .context("running systemctl --user daemon-reload")?;
    if !status.success() {
        anyhow::bail!("systemctl --user daemon-reload failed: {status}");
    }
    let status = Command::new("systemctl").args(["--user", "enable", "--now", UNIT_NAME]).status()
        .context("running systemctl --user enable --now")?;
    if !status.success() {
        anyhow::bail!("systemctl --user enable --now failed: {status}");
    }
    info!("systemd user unit enabled and started");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let unit = unit_path()?;
    let _ = Command::new("systemctl").args(["--user", "disable", "--now", UNIT_NAME]).status();
    if unit.exists() {
        std::fs::remove_file(&unit).with_context(|| format!("removing {}", unit.display()))?;
        info!(path = %unit.display(), "removed systemd user unit");
    }
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).status();
    Ok(())
}
