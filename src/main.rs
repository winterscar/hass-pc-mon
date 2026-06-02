mod cli;
mod config;
mod discovery;
mod mqtt;
mod platform;
mod sample;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let args = Cli::parse();
    let cmd = args.command.unwrap_or(Command::Run);
    println!("command: {cmd:?}");
}
