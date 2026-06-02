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
