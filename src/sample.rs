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
