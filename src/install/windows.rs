use anyhow::{Context, Result};
use std::process::Command;
use tracing::info;

const TASK_NAME: &str = "HassPcMon";

pub fn install() -> Result<()> {
    let exe = std::env::current_exe().context("resolving current exe")?;
    let exe_s = exe.display().to_string();
    let tr = format!("\"{exe_s}\" run");
    let status = Command::new("schtasks")
        .args(["/create", "/sc", "onlogon", "/tn", TASK_NAME, "/tr", &tr, "/rl", "limited", "/f"])
        .status()
        .context("running schtasks /create")?;
    if !status.success() {
        anyhow::bail!("schtasks /create failed: {status}");
    }
    info!("scheduled task created");
    // Start it now too.
    let _ = Command::new("schtasks").args(["/run", "/tn", TASK_NAME]).status();
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let status = Command::new("schtasks").args(["/delete", "/tn", TASK_NAME, "/f"]).status()
        .context("running schtasks /delete")?;
    if !status.success() {
        info!("scheduled task not present; nothing to do");
    } else {
        info!("scheduled task removed");
    }
    Ok(())
}
