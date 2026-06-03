use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub mqtt: MqttConfig,
    pub update_interval_secs: u64,
    pub idle_threshold_secs: u64,
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Write a temporary config file; tests return Result<()> so `?` propagates
    /// failures as test errors without needing the panic unwind machinery.
    fn write_tmp(contents: &str) -> Result<PathBuf> {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let path = PathBuf::from(format!("/tmp/hass-pc-mon-test-{pid}-{n}.toml"));
        std::fs::write(&path, contents)
            .with_context(|| format!("write temp config {}", path.display()))?;
        Ok(path)
    }

    #[test]
    fn parses_minimal_config() -> Result<()> {
        let p = write_tmp(r#"
update_interval_secs = 30
idle_threshold_secs = 60

[mqtt]
host = "192.168.1.10"
"#)?;
        let cfg = Config::load(&p)?;
        let _ = std::fs::remove_file(&p);
        if cfg.mqtt.host != "192.168.1.10" {
            return Err(anyhow!("host: expected 192.168.1.10, got {}", cfg.mqtt.host));
        }
        if cfg.mqtt.port != 1883 {
            return Err(anyhow!("port: expected 1883, got {}", cfg.mqtt.port));
        }
        if cfg.mqtt.username.is_some() {
            return Err(anyhow!("username: expected None"));
        }
        if cfg.update_interval_secs != 30 {
            return Err(anyhow!("update_interval_secs: expected 30, got {}", cfg.update_interval_secs));
        }
        if cfg.idle_threshold_secs != 60 {
            return Err(anyhow!("idle_threshold_secs: expected 60, got {}", cfg.idle_threshold_secs));
        }
        if cfg.topic_prefix != "hass-pc-mon" {
            return Err(anyhow!("topic_prefix: expected hass-pc-mon, got {}", cfg.topic_prefix));
        }
        if cfg.discovery_prefix != "homeassistant" {
            return Err(anyhow!("discovery_prefix: expected homeassistant, got {}", cfg.discovery_prefix));
        }
        Ok(())
    }

    #[test]
    fn rejects_zero_update_interval() -> Result<()> {
        let p = write_tmp(r#"
update_interval_secs = 0
idle_threshold_secs = 0

[mqtt]
host = "x"
"#)?;
        let result = Config::load(&p);
        let _ = std::fs::remove_file(&p);
        let err = result.unwrap_err().to_string();
        if !err.contains("update_interval_secs") {
            return Err(anyhow!("expected 'update_interval_secs' in error, got: {err}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_username_without_password() -> Result<()> {
        let p = write_tmp(r#"
update_interval_secs = 30
idle_threshold_secs = 60

[mqtt]
host = "x"
username = "u"
"#)?;
        let result = Config::load(&p);
        let _ = std::fs::remove_file(&p);
        let err = result.unwrap_err().to_string();
        if !(err.contains("username") && err.contains("password")) {
            return Err(anyhow!("expected 'username'+'password' in error, got: {err}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_missing_file() -> Result<()> {
        let result = Config::load(Path::new("/no/such/file.toml"));
        let err = result.unwrap_err().to_string();
        if !err.contains("reading config file") {
            return Err(anyhow!("expected 'reading config file' in error, got: {err}"));
        }
        Ok(())
    }
}
