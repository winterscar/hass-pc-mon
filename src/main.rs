mod cli;
mod config;
mod discovery;
mod logging;
mod mqtt;
mod platform;
mod sample;

use clap::Parser;
use cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let _log_guards = logging::init()?;
    let args = Cli::parse();
    let cmd = args.command.unwrap_or(Command::Run);
    tracing::info!(?cmd, "hass-pc-mon starting");
    Ok(())
}
