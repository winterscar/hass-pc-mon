use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct DiscoveryPayload {
    pub topic: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct Topics {
    pub availability: String,
    pub activity: String,
    pub monitor_count: String,
    pub monitor_names: String,
}

impl Topics {
    pub fn new(topic_prefix: &str, host: &str) -> Self {
        let base = format!("{topic_prefix}/{host}");
        Self {
            availability: format!("{base}/availability"),
            activity: format!("{base}/activity"),
            monitor_count: format!("{base}/monitors/count"),
            monitor_names: format!("{base}/monitors/names"),
        }
    }
}

pub fn build(discovery_prefix: &str, host: &str, topics: &Topics) -> Vec<DiscoveryPayload> {
    let unique_activity = format!("hass-pc-mon-{host}-activity");
    let unique_monitors = format!("hass-pc-mon-{host}-monitors");
    let device = serde_json::json!({
        "identifiers": [format!("hass-pc-mon-{host}")],
        "name": host,
        "manufacturer": "hass-pc-mon",
    });

    let activity_payload = serde_json::json!({
        "name": format!("{host} activity"),
        "unique_id": unique_activity,
        "object_id": unique_activity,
        "state_topic": topics.activity,
        "availability_topic": topics.availability,
        "payload_on": "ON",
        "payload_off": "OFF",
        "device_class": "occupancy",
        "device": device,
    });

    let monitors_payload = serde_json::json!({
        "name": format!("{host} monitors"),
        "unique_id": unique_monitors,
        "object_id": unique_monitors,
        "state_topic": topics.monitor_count,
        "json_attributes_topic": topics.monitor_names,
        "availability_topic": topics.availability,
        "device": device,
    });

    vec![
        DiscoveryPayload {
            topic: format!("{discovery_prefix}/binary_sensor/{unique_activity}/config"),
            payload: activity_payload,
        },
        DiscoveryPayload {
            topic: format!("{discovery_prefix}/sensor/{unique_monitors}/config"),
            payload: monitors_payload,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_are_constructed_from_prefix_and_host() {
        let t = Topics::new("hass-pc-mon", "studio-mac");
        assert_eq!(t.availability, "hass-pc-mon/studio-mac/availability");
        assert_eq!(t.activity, "hass-pc-mon/studio-mac/activity");
        assert_eq!(t.monitor_count, "hass-pc-mon/studio-mac/monitors/count");
        assert_eq!(t.monitor_names, "hass-pc-mon/studio-mac/monitors/names");
    }

    #[test]
    fn discovery_payloads_have_required_fields() {
        let topics = Topics::new("hass-pc-mon", "studio-mac");
        let payloads = build("homeassistant", "studio-mac", &topics);
        assert_eq!(payloads.len(), 2);

        let activity = &payloads[0];
        assert_eq!(activity.topic, "homeassistant/binary_sensor/hass-pc-mon-studio-mac-activity/config");
        assert_eq!(activity.payload["unique_id"], "hass-pc-mon-studio-mac-activity");
        assert_eq!(activity.payload["state_topic"], "hass-pc-mon/studio-mac/activity");
        assert_eq!(activity.payload["availability_topic"], "hass-pc-mon/studio-mac/availability");
        assert_eq!(activity.payload["payload_on"], "ON");
        assert_eq!(activity.payload["payload_off"], "OFF");

        let monitors = &payloads[1];
        assert_eq!(monitors.topic, "homeassistant/sensor/hass-pc-mon-studio-mac-monitors/config");
        assert_eq!(monitors.payload["state_topic"], "hass-pc-mon/studio-mac/monitors/count");
        assert_eq!(monitors.payload["json_attributes_topic"], "hass-pc-mon/studio-mac/monitors/names");
    }
}
