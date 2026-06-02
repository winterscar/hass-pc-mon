mod cli;
mod config;
mod discovery;
mod host;
mod install;
mod logging;
mod mqtt;
mod platform;
mod sample;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};
use config::Config;
use rumqttc::{Event, Packet};
use std::time::Duration;
use tracing::{debug, error, info, warn};

fn main() -> Result<()> {
    let _log_guards = logging::init()?;
    let args = Cli::parse();
    let cmd = args.command.unwrap_or(Command::Run);

    let config_path = match args.config {
        Some(p) => p,
        None => Config::default_path()?,
    };

    match cmd {
        Command::Run => {
            let config = Config::load(&config_path).context("loading config")?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .context("building tokio runtime")?;
            runtime.block_on(run(config))
        }
        Command::Install => install::install(),
        Command::Uninstall => install::uninstall(),
    }
}

async fn run(config: Config) -> Result<()> {
    let host = host::resolve(&config)?;
    info!(%host, broker = %config.mqtt.host, "hass-pc-mon starting run loop");

    let topics = discovery::Topics::new(&config.topic_prefix, &host);
    let discovery_payloads = discovery::build(&config.discovery_prefix, &host, &topics);

    let mqtt::MqttRuntime { mut mqtt, mut event_loop } =
        mqtt::connect(&config, &host, discovery_payloads, topics)?;

    let mut ticker = tokio::time::interval(Duration::from_secs(config.update_interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut shutdown = std::pin::pin!(tokio::signal::ctrl_c());

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("shutdown signal received");
                return Ok(());
            }
            ev = event_loop.poll() => {
                match ev {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        if let Err(e) = mqtt.on_connected().await {
                            warn!(error = ?e, "failed to publish post-connect state");
                        }
                    }
                    Ok(other) => debug!(?other, "mqtt event"),
                    Err(e) => {
                        warn!(error = %e, "mqtt event loop error — rumqttc will reconnect");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            _ = ticker.tick() => {
                let allowed = match sample::current_ssid() {
                    Ok(s) => sample::ssid_allowed(&s, &config.wifi_ssids),
                    Err(e) => {
                        warn!(error = ?e, "ssid lookup failed; skipping publish this tick");
                        continue;
                    }
                };
                if !allowed {
                    debug!("ssid not in allowed list; skipping publish");
                    continue;
                }
                match sample::take(config.idle_threshold_secs) {
                    Ok(s) => {
                        if let Err(e) = mqtt.publish_sample(&s).await {
                            warn!(error = ?e, "failed to publish sample");
                        }
                    }
                    Err(e) => {
                        warn!(error = ?e, "sample failed; skipping publish");
                    }
                }
            }
        }
    }
}
