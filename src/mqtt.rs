use crate::config::Config;
use crate::discovery::{DiscoveryPayload, Topics};
use crate::sample::Sample;
use anyhow::{Context, Result};
use rumqttc::{AsyncClient, EventLoop, LastWill, MqttOptions, QoS, Transport};
use std::time::Duration;
use tracing::{debug, info};

pub struct Mqtt {
    client: AsyncClient,
    topics: Topics,
    discovery: Vec<DiscoveryPayload>,
    discovery_published: bool,
}

pub struct MqttRuntime {
    pub mqtt: Mqtt,
    pub event_loop: EventLoop,
}

const ONLINE: &str = "online";
const OFFLINE: &str = "offline";

pub fn connect(config: &Config, host: &str, discovery: Vec<DiscoveryPayload>, topics: Topics) -> Result<MqttRuntime> {
    let client_id = format!("hass-pc-mon-{host}");
    let mut opts = MqttOptions::new(&client_id, &config.mqtt.host, config.mqtt.port);
    opts.set_keep_alive(Duration::from_secs(30));
    if let (Some(u), Some(p)) = (&config.mqtt.username, &config.mqtt.password) {
        opts.set_credentials(u, p);
    }
    if config.mqtt.tls {
        opts.set_transport(Transport::tls_with_default_config());
    }
    opts.set_last_will(LastWill::new(
        &topics.availability,
        OFFLINE.as_bytes(),
        QoS::AtLeastOnce,
        true,
    ));

    let (client, event_loop) = AsyncClient::new(opts, 16);
    Ok(MqttRuntime {
        mqtt: Mqtt {
            client,
            topics,
            discovery,
            discovery_published: false,
        },
        event_loop,
    })
}

impl Mqtt {
    /// Called once on each connection event from the event loop.
    /// Publishes discovery (always — Home Assistant restart safety) and availability=online.
    pub async fn on_connected(&mut self) -> Result<()> {
        info!("mqtt connected — publishing discovery and availability");
        for d in &self.discovery {
            let payload = serde_json::to_vec(&d.payload).context("serializing discovery payload")?;
            self.client.publish(&d.topic, QoS::AtLeastOnce, true, payload).await
                .with_context(|| format!("publishing discovery to {}", d.topic))?;
        }
        self.client.publish(&self.topics.availability, QoS::AtLeastOnce, true, ONLINE.as_bytes().to_vec()).await
            .context("publishing availability=online")?;
        self.discovery_published = true;
        Ok(())
    }

    pub async fn publish_sample(&self, sample: &Sample) -> Result<()> {
        let activity = if sample.activity { "ON" } else { "OFF" };
        debug!(activity, monitor_count = sample.monitor_count, "publishing sample");

        self.client.publish(&self.topics.activity, QoS::AtLeastOnce, true, activity.as_bytes().to_vec()).await
            .context("publishing activity")?;

        let count_bytes = sample.monitor_count.to_string().into_bytes();
        self.client.publish(&self.topics.monitor_count, QoS::AtLeastOnce, true, count_bytes).await
            .context("publishing monitors/count")?;

        let names_json = serde_json::to_vec(&sample.monitor_names).context("serializing monitor names")?;
        self.client.publish(&self.topics.monitor_names, QoS::AtLeastOnce, true, names_json).await
            .context("publishing monitors/names")?;

        Ok(())
    }
}
