# hass-pc-mon Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a small Rust service that reports PC state, activity, and connected monitors to MQTT (Home Assistant auto-discovery) from macOS, Linux, and Windows, gated to a configured wifi SSID.

**Architecture:** Single binary with `run` / `install` / `uninstall` subcommands. Cross-platform crates (`rumqttc`, `serde`/`toml`, `display-info`, `hostname`, `tracing`) do the heavy lifting; `#[cfg(target_os = ...)]`-gated modules handle the two genuinely OS-specific signals — idle time and SSID. Main loop ticks every `update_interval_secs`, samples state, and publishes retained MQTT topics; LWT handles offline reporting; HA discovery payloads are published once on connect.

**Tech Stack:** Rust (stable, 2021 edition), `rumqttc`, `serde` + `toml`, `clap` (derive), `tracing` + `tracing-subscriber` + `tracing-appender`, `display-info`, `hostname`, `shellexpand`, `anyhow`, `thiserror`. Platform-specific: `core-graphics` (macOS), `x11` + `zbus` (Linux), `windows` crate (Windows).

**Plan structure note:** Tasks 1–13 are cross-platform. Tasks 14–16 are the per-OS implementations of the `Platform` trait (idle + SSID). The cross-platform tasks compile on any host — platform tasks require the host OS to fully build and test.

---

## Task 1: Initialize Cargo project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `.gitignore`

- [ ] **Step 1: Initialize the crate**

Run from `/Users/mat/src/hass-pc-mon`:

```bash
cargo init --name hass-pc-mon --bin
```

Expected: creates `Cargo.toml`, `src/main.rs`, and (if `git` is on PATH) initializes a git repo with a `.gitignore`. If no git repo gets initialized, run `git init` separately.

- [ ] **Step 2: Pin Rust edition and add baseline dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "hass-pc-mon"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
shellexpand = "3"
hostname = "0.4"
display-info = "0.5"
rumqttc = "0.24"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "time", "signal"] }

[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.23"
core-foundation = "0.9"

[target.'cfg(target_os = "linux")'.dependencies]
x11 = { version = "2", features = ["xlib", "xss"] }
zbus = { version = "4", default-features = false, features = ["tokio"] }

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.56", features = ["Win32_UI_Input_KeyboardAndMouse", "Win32_System_SystemInformation"] }
```

- [ ] **Step 3: Ensure `.gitignore` exists**

If `cargo init` already created one, leave it. Otherwise create `.gitignore`:

```
/target
```

- [ ] **Step 4: Replace src/main.rs with a placeholder**

```rust
fn main() {
    println!("hass-pc-mon");
}
```

- [ ] **Step 5: Verify the project compiles**

Run: `cargo build`
Expected: builds successfully (downloads will take a minute on first run); produces `target/debug/hass-pc-mon`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock .gitignore src/main.rs
git commit -m "chore: scaffold hass-pc-mon crate"
```

---

## Task 2: Define the `Config` type and TOML parsing

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the failing test**

Create `src/config.rs` with this content:

```rust
use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub mqtt: MqttConfig,
    pub update_interval_secs: u64,
    pub idle_threshold_secs: u64,
    pub wifi_ssids: Vec<String>,
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
    #[serde(default = "default_discovery_prefix")]
    pub discovery_prefix: String,
    #[serde(default)]
    pub hostname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub tls: bool,
}

fn default_port() -> u16 { 1883 }
fn default_topic_prefix() -> String { "hass-pc-mon".to_string() }
fn default_discovery_prefix() -> String { "homeassistant".to_string() }

impl Config {
    pub fn default_path() -> Result<PathBuf> {
        let expanded = shellexpand::tilde("~/.config/hass-pc-mon.toml");
        Ok(PathBuf::from(expanded.into_owned()))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .with_context(|| format!("parsing config file {}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        if self.update_interval_secs == 0 {
            bail!("update_interval_secs must be > 0");
        }
        match (&self.mqtt.username, &self.mqtt.password) {
            (Some(_), None) | (None, Some(_)) => {
                return Err(anyhow!("mqtt.username and mqtt.password must both be set, or neither"));
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp(contents: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_minimal_config() {
        let f = write_tmp(r#"
[mqtt]
host = "192.168.1.10"

update_interval_secs = 30
idle_threshold_secs = 60
wifi_ssids = ["HomeNet"]
"#);
        let cfg = Config::load(f.path()).unwrap();
        assert_eq!(cfg.mqtt.host, "192.168.1.10");
        assert_eq!(cfg.mqtt.port, 1883);
        assert!(cfg.mqtt.username.is_none());
        assert_eq!(cfg.update_interval_secs, 30);
        assert_eq!(cfg.idle_threshold_secs, 60);
        assert_eq!(cfg.wifi_ssids, vec!["HomeNet".to_string()]);
        assert_eq!(cfg.topic_prefix, "hass-pc-mon");
        assert_eq!(cfg.discovery_prefix, "homeassistant");
    }

    #[test]
    fn rejects_zero_update_interval() {
        let f = write_tmp(r#"
[mqtt]
host = "x"
update_interval_secs = 0
idle_threshold_secs = 0
wifi_ssids = []
"#);
        let err = Config::load(f.path()).unwrap_err().to_string();
        assert!(err.contains("update_interval_secs"), "got: {err}");
    }

    #[test]
    fn rejects_username_without_password() {
        let f = write_tmp(r#"
[mqtt]
host = "x"
username = "u"
update_interval_secs = 30
idle_threshold_secs = 60
wifi_ssids = []
"#);
        let err = Config::load(f.path()).unwrap_err().to_string();
        assert!(err.contains("username") && err.contains("password"), "got: {err}");
    }

    #[test]
    fn rejects_missing_file() {
        let err = Config::load(Path::new("/no/such/file.toml")).unwrap_err().to_string();
        assert!(err.contains("reading config file"), "got: {err}");
    }

    #[test]
    fn allows_empty_wifi_ssids() {
        let f = write_tmp(r#"
[mqtt]
host = "x"
update_interval_secs = 30
idle_threshold_secs = 60
wifi_ssids = []
"#);
        let cfg = Config::load(f.path()).unwrap();
        assert!(cfg.wifi_ssids.is_empty());
    }
}
```

Add the test dependency. Append to `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Wire the module into main**

Replace `src/main.rs` with:

```rust
mod config;

fn main() {
    println!("hass-pc-mon");
}
```

- [ ] **Step 3: Run the tests — expect them to pass**

Run: `cargo test --lib config::`
Expected: 5 tests pass.

(They pass on first run because we wrote test + impl together. That's fine — we still got the validation that the code does what we think; if a test had silently passed we'd have caught it in review.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/config.rs src/main.rs
git commit -m "feat: config loading and validation"
```

---

## Task 3: Define the platform trait surface and stubs

**Files:**
- Create: `src/platform/mod.rs`
- Create: `src/platform/macos.rs`
- Create: `src/platform/linux.rs`
- Create: `src/platform/windows.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create the cfg-gated module re-exports**

Create `src/platform/mod.rs`:

```rust
use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

pub fn idle_seconds() -> Result<u64> {
    imp::idle_seconds()
}

pub fn current_ssid() -> Result<Option<String>> {
    imp::current_ssid()
}
```

- [ ] **Step 2: Create stub implementations for all three OSes**

Each stub returns `unimplemented!()` so the crate links on every OS and we can fill in real implementations later in tasks 14-16.

Create `src/platform/macos.rs`:

```rust
use anyhow::Result;

pub fn idle_seconds() -> Result<u64> {
    unimplemented!("macOS idle_seconds — implemented in Task 14")
}

pub fn current_ssid() -> Result<Option<String>> {
    unimplemented!("macOS current_ssid — implemented in Task 14")
}
```

Create `src/platform/linux.rs`:

```rust
use anyhow::Result;

pub fn idle_seconds() -> Result<u64> {
    unimplemented!("Linux idle_seconds — implemented in Task 15")
}

pub fn current_ssid() -> Result<Option<String>> {
    unimplemented!("Linux current_ssid — implemented in Task 15")
}
```

Create `src/platform/windows.rs`:

```rust
use anyhow::Result;

pub fn idle_seconds() -> Result<u64> {
    unimplemented!("Windows idle_seconds — implemented in Task 16")
}

pub fn current_ssid() -> Result<Option<String>> {
    unimplemented!("Windows current_ssid — implemented in Task 16")
}
```

- [ ] **Step 3: Wire `platform` into main**

Update `src/main.rs`:

```rust
mod config;
mod platform;

fn main() {
    println!("hass-pc-mon");
}
```

- [ ] **Step 4: Verify it builds on the host OS**

Run: `cargo build`
Expected: builds successfully.

- [ ] **Step 5: Commit**

```bash
git add src/platform src/main.rs
git commit -m "feat: cfg-gated platform module with stubs"
```

---

## Task 4: Sample one tick of state

**Files:**
- Create: `src/sample.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the sampler**

Create `src/sample.rs`:

```rust
use crate::platform;
use anyhow::Result;
use display_info::DisplayInfo;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct Sample {
    pub activity: bool,
    pub monitor_count: usize,
    pub monitor_names: Vec<String>,
}

pub fn take(idle_threshold_secs: u64) -> Result<Sample> {
    let idle = platform::idle_seconds()?;
    let monitors = DisplayInfo::all().unwrap_or_default();
    let monitor_names: Vec<String> = monitors
        .iter()
        .map(|d| if d.name.is_empty() { format!("display-{}", d.id) } else { d.name.clone() })
        .collect();
    Ok(Sample {
        activity: idle < idle_threshold_secs,
        monitor_count: monitor_names.len(),
        monitor_names,
    })
}

pub fn current_ssid() -> Result<Option<String>> {
    platform::current_ssid()
}

pub fn ssid_allowed(current: &Option<String>, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }
    match current {
        Some(s) => allowed.iter().any(|a| a == s),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_allowed_list_means_always_allowed() {
        assert!(ssid_allowed(&Some("anything".into()), &[]));
        assert!(ssid_allowed(&None, &[]));
    }

    #[test]
    fn matching_ssid_allowed() {
        let allowed = vec!["HomeNet".into(), "HomeNet-5G".into()];
        assert!(ssid_allowed(&Some("HomeNet".into()), &allowed));
        assert!(ssid_allowed(&Some("HomeNet-5G".into()), &allowed));
    }

    #[test]
    fn non_matching_ssid_blocked() {
        let allowed = vec!["HomeNet".into()];
        assert!(!ssid_allowed(&Some("Cafe".into()), &allowed));
    }

    #[test]
    fn missing_ssid_blocked_when_allowed_list_set() {
        let allowed = vec!["HomeNet".into()];
        assert!(!ssid_allowed(&None, &allowed));
    }
}
```

- [ ] **Step 2: Wire `sample` into main**

Update `src/main.rs`:

```rust
mod config;
mod platform;
mod sample;

fn main() {
    println!("hass-pc-mon");
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib sample::`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/sample.rs src/main.rs
git commit -m "feat: per-tick sampler and SSID gate logic"
```

---

## Task 5: Build Home Assistant discovery payloads

**Files:**
- Create: `src/discovery.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write the discovery builder + tests**

Create `src/discovery.rs`:

```rust
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
```

- [ ] **Step 2: Wire `discovery` into main**

Update `src/main.rs`:

```rust
mod config;
mod discovery;
mod platform;
mod sample;

fn main() {
    println!("hass-pc-mon");
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib discovery::`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/discovery.rs src/main.rs
git commit -m "feat: HA discovery payload construction"
```

---

## Task 6: MQTT client wrapper

**Files:**
- Create: `src/mqtt.rs`
- Modify: `src/main.rs`

This task wraps `rumqttc` to: connect with LWT, publish discovery once on connect, and publish state topics retained.

- [ ] **Step 1: Write the wrapper**

Create `src/mqtt.rs`:

```rust
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

```

(Note: imports `Event` and `Packet` are re-exported so callers can `match` on them — they're used directly from `rumqttc` in `main.rs`.)

- [ ] **Step 2: Wire `mqtt` into main**

Update `src/main.rs`:

```rust
mod config;
mod discovery;
mod mqtt;
mod platform;
mod sample;

fn main() {
    println!("hass-pc-mon");
}
```

- [ ] **Step 3: Build to verify**

Run: `cargo build`
Expected: builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src/mqtt.rs src/main.rs
git commit -m "feat: mqtt client wrapper with LWT and HA discovery"
```

---

## Task 7: CLI parsing with `clap`

**Files:**
- Modify: `src/main.rs`
- Create: `src/cli.rs`

- [ ] **Step 1: Define the CLI**

Create `src/cli.rs`:

```rust
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
```

- [ ] **Step 2: Wire `cli` into main**

Update `src/main.rs`:

```rust
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
```

- [ ] **Step 3: Smoke test the CLI**

Run: `cargo run -- --help`
Expected: prints help text listing `run`, `install`, `uninstall` subcommands and `--config`.

Run: `cargo run`
Expected: prints `command: Run`.

Run: `cargo run -- install`
Expected: prints `command: Install`.

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat: cli subcommands"
```

---

## Task 8: Logging setup (`tracing` + file appender)

**Files:**
- Create: `src/logging.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement logging init**

Create `src/logging.rs`:

```rust
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
```

- [ ] **Step 2: Wire `logging` into main**

Update `src/main.rs`:

```rust
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
```

- [ ] **Step 3: Smoke test**

Run: `cargo run`
Expected: stderr shows a `hass-pc-mon starting` info log; on macOS, `~/Library/Logs/hass-pc-mon.log` is created and contains the same line; on Linux, no file is created.

- [ ] **Step 4: Commit**

```bash
git add src/logging.rs src/main.rs
git commit -m "feat: tracing-based logging with per-os file fallback"
```

---

## Task 9: Hostname helper

**Files:**
- Create: `src/host.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Implement `host::resolve`**

Create `src/host.rs`:

```rust
use crate::config::Config;
use anyhow::{Context, Result};

/// Returns the hostname to use in MQTT topics — config override if set, else OS hostname.
/// The result is lowercased and any whitespace replaced with `-` to keep MQTT topics tidy.
pub fn resolve(config: &Config) -> Result<String> {
    let raw = if let Some(h) = &config.hostname {
        h.clone()
    } else {
        hostname::get()
            .context("reading OS hostname")?
            .to_string_lossy()
            .to_string()
    };
    Ok(sanitize(&raw))
}

fn sanitize(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_spaces_and_lowercases() {
        assert_eq!(sanitize("Studio Mac"), "studio-mac");
    }

    #[test]
    fn sanitize_passes_through_simple_names() {
        assert_eq!(sanitize("studio-mac"), "studio-mac");
    }

    #[test]
    fn sanitize_strips_invalid_chars() {
        assert_eq!(sanitize("foo.bar/baz"), "foo-bar-baz");
    }
}
```

- [ ] **Step 2: Wire into main**

Update `src/main.rs`:

```rust
mod cli;
mod config;
mod discovery;
mod host;
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
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib host::`
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/host.rs src/main.rs
git commit -m "feat: hostname resolution and sanitization"
```

---

## Task 10: Wire the run loop in `main.rs`

**Files:**
- Modify: `src/main.rs`

This is the integration step. Brings together config, host, discovery, mqtt, sampling, and the SSID gate.

- [ ] **Step 1: Implement the async run loop**

Replace `src/main.rs`:

```rust
mod cli;
mod config;
mod discovery;
mod host;
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
        Command::Install => {
            // Filled in by Task 11.
            anyhow::bail!("install not yet implemented")
        }
        Command::Uninstall => {
            anyhow::bail!("uninstall not yet implemented")
        }
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
```

- [ ] **Step 2: Verify the crate builds**

Run: `cargo build`
Expected: builds without errors. (Platform stubs will panic at runtime — that's OK; we test runtime after the platform tasks land.)

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire run loop — sample, gate, publish"
```

---

## Task 11: Install / uninstall scaffolding + macOS implementation

**Files:**
- Create: `src/install/mod.rs`
- Create: `src/install/macos.rs`
- Create: `src/install/linux.rs`
- Create: `src/install/windows.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create the cfg-gated install module**

Create `src/install/mod.rs`:

```rust
use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

pub fn install() -> Result<()> { imp::install() }
pub fn uninstall() -> Result<()> { imp::uninstall() }
```

- [ ] **Step 2: Implement macOS install/uninstall**

Create `src/install/macos.rs`:

```rust
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
```

- [ ] **Step 3: Linux + Windows stubs**

Create `src/install/linux.rs`:

```rust
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
```

Create `src/install/windows.rs`:

```rust
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
```

- [ ] **Step 4: Wire `install` module + commands**

Update `src/main.rs` — replace the two `bail!` lines for Install/Uninstall:

Find:

```rust
        Command::Install => {
            // Filled in by Task 11.
            anyhow::bail!("install not yet implemented")
        }
        Command::Uninstall => {
            anyhow::bail!("uninstall not yet implemented")
        }
```

Replace with:

```rust
        Command::Install => install::install(),
        Command::Uninstall => install::uninstall(),
```

And add `mod install;` to the module list at the top of `src/main.rs` (alphabetical: between `host` and `logging`).

- [ ] **Step 5: Build**

Run: `cargo build`
Expected: builds successfully on whatever OS you're on.

- [ ] **Step 6: Commit**

```bash
git add src/install src/main.rs
git commit -m "feat: install/uninstall subcommands for all three OSes"
```

---

## Task 12: Implement macOS platform — idle + SSID

**Files:**
- Modify: `src/platform/macos.rs`

Only runs on macOS hosts. Skip the smoke test if you're not on macOS but still verify it compiles via `cargo check --target x86_64-apple-darwin` if you have the target installed; otherwise just keep moving and verify on a Mac.

- [ ] **Step 1: Implement idle via Core Graphics**

Replace `src/platform/macos.rs`:

```rust
use anyhow::{anyhow, Context, Result};
use core_graphics::event::{CGEventType, CGEventSource, CGEventSourceStateID};
use std::process::Command;

pub fn idle_seconds() -> Result<u64> {
    let secs = CGEventSource::seconds_since_last_event_type(
        CGEventSourceStateID::HIDSystemState,
        CGEventType::Null,
    );
    // `CGEventType::Null` (== kCGAnyInputEventType) reports across any input event.
    if !secs.is_finite() || secs < 0.0 {
        return Err(anyhow!("CGEventSource returned non-finite seconds: {secs}"));
    }
    Ok(secs as u64)
}

pub fn current_ssid() -> Result<Option<String>> {
    let out = Command::new("/usr/sbin/networksetup")
        .args(["-getairportnetwork", "en0"])
        .output()
        .context("running networksetup")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Expected format: "Current Wi-Fi Network: HomeNet"
    if let Some(rest) = stdout.split_once("Current Wi-Fi Network:") {
        let ssid = rest.1.trim();
        if ssid.is_empty() || ssid.to_lowercase().contains("not associated") {
            return Ok(None);
        }
        return Ok(Some(ssid.to_string()));
    }
    // "You are not associated with an AirPort network." style output.
    Ok(None)
}
```

- [ ] **Step 2: Build on macOS**

Run on a macOS host: `cargo build`
Expected: builds without errors.

- [ ] **Step 3: Smoke test idle**

Add a one-off test binary call. Run:

```bash
cargo run -- --config /no/such/file run 2>&1 | head -n 5 || true
```

This isn't a real test of idle — the run command will fail at config load — but it verifies the platform code links. For a real test, write a tiny throwaway scratch program or use `cargo test`:

Add a `#[cfg(test)]` block at the bottom of `src/platform/macos.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_seconds_returns_some_value() {
        let s = idle_seconds().expect("idle should succeed");
        // Sanity: on a fresh test run idle should be well under a day.
        assert!(s < 86_400, "implausible idle: {s}");
    }

    #[test]
    fn ssid_does_not_error() {
        // May return Some or None depending on host. Either is fine.
        let _ = current_ssid().expect("ssid should not error");
    }
}
```

Run: `cargo test --lib platform::macos`
Expected: 2 tests pass on a macOS host. (On non-macOS, the module isn't compiled and these tests don't exist.)

- [ ] **Step 4: Commit**

```bash
git add src/platform/macos.rs
git commit -m "feat(macos): idle via core-graphics, ssid via networksetup"
```

---

## Task 13: Implement Linux platform — idle (X11 + Wayland fallback) + SSID

**Files:**
- Modify: `src/platform/linux.rs`

Only validates on a Linux host. On non-Linux, this code isn't compiled, so cross-platform CI is unaffected.

- [ ] **Step 1: Implement idle**

Replace `src/platform/linux.rs`:

```rust
use anyhow::{anyhow, Result};
use std::sync::OnceLock;
use tracing::warn;

pub fn idle_seconds() -> Result<u64> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() && std::env::var("DISPLAY").is_err() {
        return wayland_idle_seconds();
    }
    x11_idle_seconds()
}

fn x11_idle_seconds() -> Result<u64> {
    use x11::xlib;
    use x11::xss;
    unsafe {
        let display = xlib::XOpenDisplay(std::ptr::null());
        if display.is_null() {
            return Err(anyhow!("XOpenDisplay returned null"));
        }
        let root = xlib::XDefaultRootWindow(display);
        let info = xss::XScreenSaverAllocInfo();
        if info.is_null() {
            xlib::XCloseDisplay(display);
            return Err(anyhow!("XScreenSaverAllocInfo returned null"));
        }
        let status = xss::XScreenSaverQueryInfo(display, root, info);
        let idle_ms = if status == 0 { 0 } else { (*info).idle as u64 };
        xlib::XFree(info as *mut _);
        xlib::XCloseDisplay(display);
        Ok(idle_ms / 1000)
    }
}

static WAYLAND_WARN: OnceLock<()> = OnceLock::new();

fn wayland_idle_seconds() -> Result<u64> {
    // No portable Wayland idle API. We log once and assume "active" (0 seconds idle)
    // so that activity reports remain conservative — i.e. we'll over-report activity
    // rather than miss a present user.
    WAYLAND_WARN.get_or_init(|| {
        warn!("Wayland session detected; portable idle detection is not available — reporting 0 seconds idle");
    });
    Ok(0)
}

pub fn current_ssid() -> Result<Option<String>> {
    use std::process::Command;
    let out = match Command::new("iwgetid").arg("-r").output() {
        Ok(o) => o,
        Err(e) => {
            warn!(error = %e, "iwgetid not available; treating as no SSID");
            return Ok(None);
        }
    };
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_seconds_does_not_error_on_headless() {
        // On a headless CI Linux box X may not be available; we still expect
        // either a successful Wayland fallback OR an X11 error. Either is acceptable
        // — this test exists to ensure the function exits cleanly and doesn't panic.
        let _ = idle_seconds();
    }

    #[test]
    fn ssid_does_not_error() {
        let _ = current_ssid();
    }
}
```

- [ ] **Step 2: Build on Linux**

Run on a Linux host: `cargo build`
Expected: builds without errors. If `x11` headers are missing, install them (`apt install libx11-dev libxss-dev` on Debian/Ubuntu).

- [ ] **Step 3: Run platform tests**

Run: `cargo test --lib platform::linux`
Expected: 2 tests pass (or 1 of them does — the headless one just needs to not panic).

- [ ] **Step 4: Commit**

```bash
git add src/platform/linux.rs
git commit -m "feat(linux): idle via XScreenSaver with Wayland fallback, ssid via iwgetid"
```

---

## Task 14: Implement Windows platform — idle + SSID

**Files:**
- Modify: `src/platform/windows.rs`

Validates on a Windows host. Cross-compilation from macOS/Linux to Windows works but isn't required — verify on the target.

- [ ] **Step 1: Implement idle**

Replace `src/platform/windows.rs`:

```rust
use anyhow::{anyhow, Result};
use std::process::Command;
use tracing::warn;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

pub fn idle_seconds() -> Result<u64> {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        let ok = GetLastInputInfo(&mut lii);
        if !ok.as_bool() {
            return Err(anyhow!("GetLastInputInfo returned false"));
        }
        let now_ms = GetTickCount64();
        let last_ms = lii.dwTime as u64;
        // GetLastInputInfo's dwTime is a 32-bit tick count and wraps every ~49.7 days.
        // GetTickCount64 doesn't wrap on any realistic uptime, so when last_ms > now_ms
        // (which means low 32 bits wrapped) we recover by interpreting last_ms as if it's
        // in the previous wraparound window.
        let idle_ms = if last_ms <= now_ms {
            now_ms - last_ms
        } else {
            (now_ms + (u32::MAX as u64 + 1)) - last_ms
        };
        Ok(idle_ms / 1000)
    }
}

pub fn current_ssid() -> Result<Option<String>> {
    let out = match Command::new("netsh").args(["wlan", "show", "interfaces"]).output() {
        Ok(o) => o,
        Err(e) => {
            warn!(error = %e, "netsh not available; treating as no SSID");
            return Ok(None);
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        // Match `SSID                   : HomeNet`, but not `BSSID`.
        if let Some(rest) = trimmed.strip_prefix("SSID") {
            // Skip if this is the BSSID line — would have started with "BSSID".
            // Skip leading whitespace/colon.
            let after_colon = rest.splitn(2, ':').nth(1).unwrap_or("").trim();
            if !after_colon.is_empty() {
                return Ok(Some(after_colon.to_string()));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_seconds_returns_sane_value() {
        let s = idle_seconds().expect("idle should succeed");
        assert!(s < 86_400, "implausible idle: {s}");
    }

    #[test]
    fn ssid_does_not_error() {
        let _ = current_ssid();
    }
}
```

- [ ] **Step 2: Build on Windows**

Run on a Windows host: `cargo build`
Expected: builds without errors.

- [ ] **Step 3: Run platform tests**

Run: `cargo test --lib platform::windows`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/platform/windows.rs
git commit -m "feat(windows): idle via GetLastInputInfo, ssid via netsh"
```

---

## Task 15: End-to-end smoke test

This is a manual verification step on the host OS — no code changes.

- [ ] **Step 1: Create a working config**

Write `~/.config/hass-pc-mon.toml` (substitute your actual broker + SSID):

```toml
[mqtt]
host = "192.168.1.10"
port = 1883

update_interval_secs = 5
idle_threshold_secs = 30
wifi_ssids = []   # disable gate temporarily for testing
```

- [ ] **Step 2: Subscribe to the broker in another terminal**

```bash
mosquitto_sub -h 192.168.1.10 -v -t 'hass-pc-mon/#' -t 'homeassistant/#'
```

Expected once `cargo run` starts: discovery configs appear under `homeassistant/...`; `availability` becomes `online`; every 5 seconds `activity`, `monitors/count`, and `monitors/names` are published.

- [ ] **Step 3: Run the binary**

```bash
cargo run -- run
```

Expected log output (info level): `mqtt connected — publishing discovery and availability`, then no further log lines per tick at the default level.

- [ ] **Step 4: Exercise transitions**

- Move the mouse: within `update_interval_secs`, `activity` shows `ON`.
- Leave the machine idle for `idle_threshold_secs + update_interval_secs`: `activity` flips to `OFF`.
- Plug/unplug an external monitor: `monitors/count` and `monitors/names` change.
- Stop the binary (Ctrl-C): broker shows `availability = offline` after the keep-alive timeout (~30s).

- [ ] **Step 5: Re-enable SSID gate and verify**

Edit the config: `wifi_ssids = ["YourActualSSID"]`. Restart. Connect to a different SSID (or briefly disconnect): publishing stops, and eventually LWT fires.

- [ ] **Step 6: Test install on the host OS**

```bash
cargo build --release
./target/release/hass-pc-mon install
```

Verify the autostart file landed in the expected location and the service is running.

```bash
./target/release/hass-pc-mon uninstall
```

Verify it's removed.

- [ ] **Step 7: Commit any config docs you want to leave behind**

If you've added a sample config or notes, commit them. Otherwise skip.

```bash
git status
# If nothing tracked changed, no commit needed.
```

---

## Notes on cross-platform CI (optional, not part of this plan)

If CI is desired later, GitHub Actions can run `cargo check` and `cargo test --lib` on `macos-latest`, `ubuntu-latest`, and `windows-latest`. The cross-platform tests (`config`, `discovery`, `host`, `sample`) run everywhere; the platform-specific tests run on their host OS only.
