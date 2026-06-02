# hass-pc-mon — Design

A small cross-platform service that reports PC state, connected monitors, and user activity over MQTT (with Home Assistant auto-discovery), gated to a designated home wifi.

## Goals

- Run on macOS, Linux, and Windows from a single Rust codebase.
- Start automatically on user login on each OS.
- While the user is logged in and the machine is reachable on a configured wifi network, publish:
  - **availability** — on/off, via MQTT Last Will (LWT).
  - **activity** — user is actively using mouse/keyboard, derived from OS idle-time APIs.
  - **monitors** — count and names of connected displays.
- Read configuration from `~/.config/hass-pc-mon.toml`.
- Gate publishing on the current wifi SSID matching one of the configured SSIDs.

## Non-goals

- Reporting CPU / memory / disk / network metrics.
- TLS client certificates (basic TLS is in scope; mutual TLS is not).
- Multiple-broker fallback.
- A GUI or config editor.
- Auto-updates.
- Integration tests against a real MQTT broker.

## High-level architecture

Single binary with three subcommands:

- `hass-pc-mon` (default) / `hass-pc-mon run` — run the publish loop.
- `hass-pc-mon install` — write per-OS autostart definition and enable it.
- `hass-pc-mon uninstall` — reverse `install`.

Crate layout:

```
hass-pc-mon/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI entry, top-level loop
│   ├── config.rs        # TOML load + validation
│   ├── mqtt.rs          # Connect, LWT, publish loop
│   ├── discovery.rs     # Build HA discovery payloads
│   ├── sample.rs        # Sample one tick of state
│   ├── install.rs       # Per-OS autostart install / uninstall
│   └── platform/
│       ├── mod.rs       # cfg-gated re-exports
│       ├── macos.rs     # idle_seconds, current_ssid
│       ├── linux.rs     # idle_seconds, current_ssid
│       └── windows.rs   # idle_seconds, current_ssid
```

Cross-platform crates do the heavy lifting:

- `rumqttc` — MQTT client with native reconnect / backoff.
- `serde` + `toml` — config.
- `display-info` — connected monitors (cross-platform).
- `hostname` — host name.
- `tracing` + `tracing-subscriber` — logging.
- `clap` (derive) — CLI.

Custom per-OS code, gated with `#[cfg(target_os = "...")]`, only for the two signals where cross-platform crates are thin or broken: **idle time** and **SSID**.

## Main loop

```
load config
build mqtt client with LWT: <prefix>/<host>/availability = "offline" (retained)
connect to broker (retries forever with backoff if unreachable)
on connect:
    publish HA discovery configs (retained, one-shot)
    publish availability = "online" (retained)
loop every update_interval_secs:
    sample current_ssid
    if configured wifi_ssids is non-empty AND current_ssid not in wifi_ssids:
        skip publish this tick
        continue
    sample idle_seconds, monitors
    publish activity + monitors topics (retained)
    # availability is NOT republished per tick — set once on connect, cleared by LWT on disconnect
```

Notes:
- The broker is LAN-only, so off-network → broker unreachable → LWT fires automatically; the SSID check is a belt-and-braces guard for when the machine is on a reachable-but-not-home LAN (e.g. office VPN).
- `wifi_ssids = []` disables the SSID gate.

## MQTT topics

State topics (retained, published each tick):

| Topic                                          | Payload                  |
|------------------------------------------------|--------------------------|
| `<prefix>/<host>/availability`                 | `online` / `offline`     |
| `<prefix>/<host>/activity`                     | `ON` / `OFF`             |
| `<prefix>/<host>/monitors/count`               | integer as string        |
| `<prefix>/<host>/monitors/names`               | JSON array of strings    |

`<prefix>` defaults to `hass-pc-mon`. `<host>` defaults to the OS hostname (overridable in config).

Home Assistant discovery (retained, published once on each connect):

- `<discovery_prefix>/binary_sensor/hass-pc-mon-<host>-activity/config`
  - `device_class: "occupancy"`, payload `ON`/`OFF`, `availability_topic` = host's availability topic.
- `<discovery_prefix>/sensor/hass-pc-mon-<host>-monitors/config`
  - `state_topic` = `monitors/count`, `json_attributes_topic` = `monitors/names`, `availability_topic` = host's availability topic.

`<discovery_prefix>` defaults to `homeassistant`.

The on/off state of the PC itself is conveyed via the `availability_topic` on each entity — HA shows entities as unavailable when the PC is off — so no separate "state" entity is required.

`activity` is derived from `idle_seconds < idle_threshold_secs`.

## Configuration

`~/.config/hass-pc-mon.toml`:

```toml
[mqtt]
host = "192.168.1.10"        # required
port = 1883                  # optional, default 1883
username = "..."             # optional
password = "..."             # optional
tls = false                  # optional, default false

update_interval_secs = 30                # required, > 0
idle_threshold_secs  = 60                # required, >= 0
wifi_ssids = ["HomeNet", "HomeNet-5G"]   # required; [] disables SSID gate

# Optional overrides:
topic_prefix     = "hass-pc-mon"
discovery_prefix = "homeassistant"
hostname         = "studio-mac"          # default: OS hostname
```

Validation at load time:
- Missing file → exit non-zero with a clear error message.
- `update_interval_secs <= 0` or `idle_threshold_secs < 0` → error.
- `username` without `password`, or vice versa → error.
- `wifi_ssids` must be present; may be empty.

The config path is the literal `~/.config/hass-pc-mon.toml` on all three OSes — `~` is expanded to the user's home directory (`$HOME` on macOS/Linux, `%USERPROFILE%` on Windows). On Windows this yields e.g. `C:\Users\<user>\.config\hass-pc-mon.toml`. We deliberately do not use platform-native config dirs (`%APPDATA%`, etc.) because the requirement is a single path that works everywhere.

## Platform implementations

Trait surface (`src/platform/mod.rs`):

```rust
pub fn idle_seconds() -> Result<u64>;
pub fn current_ssid() -> Result<Option<String>>;
```

`connected_monitors() -> Result<Vec<String>>` and `hostname() -> Result<String>` are cross-platform (via `display-info` and `hostname` crates respectively) and live outside the `platform` module.

### Idle time

- **macOS:** `CGEventSourceSecondsSinceLastEventType(kCGEventSourceStateHIDSystemState, kCGAnyInputEventType)` via `core-graphics`. Returns `f64` seconds. No permissions required.
- **Linux (X11):** `XScreenSaverQueryInfo` via the `x11` crate.
  - **Wayland fallback:** attempt `org.freedesktop.ScreenSaver.GetSessionIdleTime` over D-Bus; on failure, log a one-shot warning and report idle = 0 (i.e. "active"). Documented limitation.
- **Windows:** `GetLastInputInfo` from `user32.dll` via the `windows` crate. Idle = `GetTickCount64() - dwTime`, in ms.

### SSID

Shell out — avoids per-OS permission/entitlement headaches:

- **macOS:** `/usr/sbin/networksetup -getairportnetwork en0`. `"You are not associated…"` → `None`.
- **Linux:** `iwgetid -r`. Empty output → `None`.
- **Windows:** `netsh wlan show interfaces`, parse the `SSID` line. Empty or "There is no wireless interface" → `None`.

### Monitors

`display_info::DisplayInfo::all()` → publish `len()` and JSON-encoded names.

## Autostart (`install` / `uninstall`)

Resolves the current binary via `std::env::current_exe()` and writes a per-OS user-session autostart definition:

- **macOS:** `~/Library/LaunchAgents/com.hass-pc-mon.plist`, then `launchctl load`.
  - `RunAtLoad=true`, `KeepAlive=true`, `StandardErrorPath=~/Library/Logs/hass-pc-mon.log`.
- **Linux:** `~/.config/systemd/user/hass-pc-mon.service`, then `systemctl --user daemon-reload && systemctl --user enable --now hass-pc-mon`.
  - `Restart=on-failure`, journal-based logging.
- **Windows:** `schtasks /create /sc onlogon /tn HassPcMon /tr "<exe>" /rl limited /f`.
  - Logging is handled inside the binary (see Logging section) — no shell-level redirection needed, since `schtasks` doesn't redirect stderr cleanly.

Both commands are idempotent: install overwrites an existing definition; uninstall ignores "not found".

## Error handling

Three categories:

1. **Fatal at startup** — config missing or invalid. Log to stderr, exit non-zero. The autostart layer will retry, but a broken config will keep failing until the user fixes it.
2. **Transient at runtime** — broker unreachable, off-wifi, single sample failure. Log at `warn`, continue. `rumqttc` reconnects with backoff.
3. **Per-signal failure** — e.g. `iwgetid` not installed. Log once at `warn`, treat that sample as "unknown" and skip publishing for that tick. Don't tear down the loop.

## Logging

`tracing` + `tracing-subscriber`, env-controlled level (`RUST_LOG`), default `info`.

The binary always writes logs to stderr. In addition, on **macOS** and **Windows** it also writes to a per-OS log file (via `tracing-appender`'s file writer) so that headless autostart runs have a discoverable log location:

- macOS: `~/Library/Logs/hass-pc-mon.log` (also referenced from the LaunchAgent plist as `StandardErrorPath` for any pre-`tracing` panic output).
- Linux: stderr only — journald captures it (`journalctl --user -u hass-pc-mon`).
- Windows: `%LOCALAPPDATA%\hass-pc-mon\hass-pc-mon.log`.

The log file path can be overridden by the `HASS_PC_MON_LOG` env var; if set to empty string, file logging is disabled.

## Testing

Proportional to project size.

- **Unit tests** — pure code only:
  - Config parsing: valid file, missing fields, wrong types, `username` without `password`, intervals out of range.
  - Discovery payload construction: correct topic and JSON shape.
  - SSID gate decision given a sampled SSID and configured list.
- **Manual platform smoke test** per OS:
  1. Run the binary with a real config.
  2. `mosquitto_sub -v -t 'hass-pc-mon/#'` shows expected topics.
  3. Lock screen → `activity` flips to `OFF` within the next tick.
  4. Plug/unplug a monitor → `monitors/count` updates.
  5. Disconnect wifi / switch to non-home SSID → publishes stop; broker eventually shows `availability = offline` after LWT timeout.
- **No integration tests against a real broker** in CI. Documented deliberate choice.

## Open questions

None at design time. Implementation may surface platform-specific issues (e.g. Wayland idle detection on uncommon compositors) which will be handled per the "per-signal failure" rule.
