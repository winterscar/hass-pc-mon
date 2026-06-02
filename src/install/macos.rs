use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

const LABEL: &str = "com.hass-pc-mon";

fn plist_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set")?;
    let mut p = PathBuf::from(home);
    p.push("Library/LaunchAgents");
    std::fs::create_dir_all(&p).context("creating LaunchAgents dir")?;
    p.push(format!("{LABEL}.plist"));
    Ok(p)
}

fn log_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set")?;
    let mut p = PathBuf::from(home);
    p.push("Library/Logs/hass-pc-mon.log");
    Ok(p)
}

pub fn install() -> Result<()> {
    let exe = std::env::current_exe().context("resolving current exe")?;
    let plist = plist_path()?;
    let log = log_path()?;
    let exe_s = exe.display().to_string();
    let log_s = log.display().to_string();

    let contents = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe_s}</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardErrorPath</key><string>{log_s}</string>
  <key>StandardOutPath</key><string>{log_s}</string>
</dict>
</plist>
"#);

    std::fs::write(&plist, contents).with_context(|| format!("writing {}", plist.display()))?;
    info!(path = %plist.display(), "wrote LaunchAgent");

    // Bootstrap into the current user's launchd domain. Ignore "already loaded".
    let _ = Command::new("launchctl").args(["unload", plist.to_str().unwrap()]).status();
    let status = Command::new("launchctl")
        .args(["load", plist.to_str().unwrap()])
        .status()
        .context("running launchctl load")?;
    if !status.success() {
        anyhow::bail!("launchctl load failed with status {status}");
    }
    info!("LaunchAgent loaded");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let plist = plist_path()?;
    if plist.exists() {
        let _ = Command::new("launchctl").args(["unload", plist.to_str().unwrap()]).status();
        std::fs::remove_file(&plist).with_context(|| format!("removing {}", plist.display()))?;
        info!(path = %plist.display(), "removed LaunchAgent");
    } else {
        info!("LaunchAgent not present; nothing to do");
    }
    Ok(())
}
