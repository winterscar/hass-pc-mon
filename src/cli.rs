use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "hass-pc-mon", version, about = "Report PC state to MQTT/Home Assistant")]
pub struct Cli {
    /// Path to the config file. Defaults to ~/.config/hass-pc-mon.toml.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the publish loop (default).
    Run,
    /// Install autostart definition for the current user.
    Install,
    /// Remove autostart definition for the current user.
    Uninstall,
}
