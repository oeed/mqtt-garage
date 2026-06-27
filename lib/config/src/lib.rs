use std::{borrow::Cow, net::SocketAddrV4};

use serde::{Deserialize, Deserializer};
#[derive(Debug, Deserialize)]
pub struct Config {
  pub wifi: WifiConfig,
  pub mqtt: MqttConfig,
  pub door: DoorConfig,
}


#[derive(Debug, Deserialize)]
pub struct WifiConfig {
  pub hostname: Cow<'static, str>,
  pub ssid: Cow<'static, str>,
  pub password: Cow<'static, str>,
  /// Syslog server to stream logs to over TCP.
  pub syslog_server: SocketAddrV4,
  /// NTP server used to obtain wall-clock time (the device has no battery-backed RTC).
  /// Point this at a LAN source (router/Home Assistant) or a public pool.
  #[serde(default = "default_ntp_server")]
  pub ntp_server: Cow<'static, str>,
  /// Upper bound on a single association/DHCP attempt. esp-idf-svc already bounds its own
  /// waits (~15s), but `start()` is unbounded; this caps the whole bring-up step.
  #[serde(default = "default_connect_timeout", deserialize_with = "deserialize_duration_millis")]
  pub connect_timeout: embassy_time::Duration,
  /// Consecutive failed association attempts (boot or reconnect) before giving up and
  /// rebooting, so a wedged link never leaves the device offline indefinitely.
  #[serde(default = "default_connect_max_attempts")]
  pub connect_max_attempts: u8,
  /// Initial backoff between association attempts; doubles up to `reconnect_max_backoff`.
  #[serde(default = "default_reconnect_backoff", deserialize_with = "deserialize_duration_millis")]
  pub reconnect_backoff: embassy_time::Duration,
  /// Cap on the exponential reconnect backoff.
  #[serde(default = "default_reconnect_max_backoff", deserialize_with = "deserialize_duration_millis")]
  pub reconnect_max_backoff: embassy_time::Duration,
  /// If MQTT stays disconnected for longer than this, reboot. Backstop for the case where
  /// the radio is associated but there is no usable path to the broker (dead AP backhaul).
  #[serde(default = "default_connectivity_timeout", deserialize_with = "deserialize_duration_millis")]
  pub connectivity_timeout: embassy_time::Duration,
}

fn default_ntp_server() -> Cow<'static, str> {
  Cow::Borrowed("pool.ntp.org")
}

fn default_connect_timeout() -> embassy_time::Duration {
  embassy_time::Duration::from_secs(20)
}

fn default_connect_max_attempts() -> u8 {
  5
}

fn default_reconnect_backoff() -> embassy_time::Duration {
  embassy_time::Duration::from_secs(1)
}

fn default_reconnect_max_backoff() -> embassy_time::Duration {
  embassy_time::Duration::from_secs(30)
}

fn default_connectivity_timeout() -> embassy_time::Duration {
  embassy_time::Duration::from_secs(120)
}


#[derive(Debug, Deserialize)]
pub struct MqttConfig {
  pub url: Cow<'static, str>,
  pub client_id: Cow<'static, str>,
  pub availability_topic: Cow<'static, str>,
  pub online_availability: Cow<'static, str>,
  pub offline_availability: Cow<'static, str>,
}


#[derive(Debug, Deserialize)]
pub struct RemoteConfig {
  #[serde(deserialize_with = "deserialize_duration_millis")]
  pub pressed_duration: embassy_time::Duration,
  #[serde(deserialize_with = "deserialize_duration_millis")]
  pub wait_duration: embassy_time::Duration,
  #[serde(deserialize_with = "deserialize_duration_millis")]
  pub max_latency_duration: embassy_time::Duration,
}

#[derive(Debug, Deserialize)]
pub struct DoorConfig {
  pub remote: RemoteConfig,

  pub command_topic: Cow<'static, str>,
  pub initial_target_state: Cow<'static, str>,
  pub state_topic: Cow<'static, str>,
  pub stuck_topic: Cow<'static, str>,
  #[serde(deserialize_with = "deserialize_duration_millis")]
  pub travel_duration: embassy_time::Duration,
  /// Minimum time a *changed* contact-sensor reading must persist before it is acted on. Filters brief
  /// reed-switch flicker (mid-travel double-triggers and stationary blips on a marginal magnet). Optional
  /// in the TOML; defaults to 1500ms. It is added to every confirm/travel window, so it can be raised
  /// without separately widening those windows.
  #[serde(default = "default_debounce_duration", deserialize_with = "deserialize_duration_millis")]
  pub debounce_duration: embassy_time::Duration,
  pub open_sensor_topic: Cow<'static, str>,
  pub closed_sensor_topic: Cow<'static, str>,
  pub safe_to_close_topic: Cow<'static, str>,
  pub max_attempts: u8,
}


fn deserialize_duration_millis<'de, D>(deserializer: D) -> Result<embassy_time::Duration, D::Error>
where
  D: Deserializer<'de>,
{
  let millis: u64 = Deserialize::deserialize(deserializer)?;
  Ok(embassy_time::Duration::from_millis(millis))
}

fn default_debounce_duration() -> embassy_time::Duration {
  embassy_time::Duration::from_millis(1500)
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_deserialize_config() {
    let config_str = r#"
[wifi]
hostname = "garage"
ssid = "my-ssid"
password = "my-password"
syslog_server = "192.168.1.10:514"

[mqtt]
url = "mqtt://localhost:1883"
client_id = "garage-door"
availability_topic = "garage/availability"
online_availability = "online"
offline_availability = "offline"

[door]
command_topic = "garage/door/command"
initial_target_state = "closed"
state_topic = "garage/door/state"
stuck_topic = "garage/door/stuck"
travel_duration = 30000
open_sensor_topic = "garage/door/open_sensor"
closed_sensor_topic = "garage/door/closed_sensor"
safe_to_close_topic = "garage/door/safe_to_close"
max_attempts = 3

[door.remote]
pressed_duration = 500
wait_duration = 1000
max_latency_duration = 250
"#;

    let config: Config = toml::from_str(config_str).unwrap();

    assert_eq!(config.wifi.ssid, "my-ssid");
    assert_eq!(config.wifi.password, "my-password");
    // omitted from the TOML above, so these fall back to their defaults
    assert_eq!(config.wifi.ntp_server, "pool.ntp.org");
    assert_eq!(config.wifi.connect_timeout, embassy_time::Duration::from_secs(20));
    assert_eq!(config.wifi.connect_max_attempts, 5);
    assert_eq!(config.wifi.reconnect_backoff, embassy_time::Duration::from_secs(1));
    assert_eq!(config.wifi.reconnect_max_backoff, embassy_time::Duration::from_secs(30));
    assert_eq!(config.wifi.connectivity_timeout, embassy_time::Duration::from_secs(120));

    assert_eq!(config.mqtt.url, "mqtt://localhost:1883");
    assert_eq!(config.mqtt.client_id, "garage-door");
    assert_eq!(config.mqtt.availability_topic, "garage/availability");
    assert_eq!(config.mqtt.online_availability, "online");
    assert_eq!(config.mqtt.offline_availability, "offline");

    assert_eq!(config.door.command_topic, "garage/door/command");
    assert_eq!(config.door.initial_target_state, "closed");
    assert_eq!(config.door.state_topic, "garage/door/state");
    assert_eq!(config.door.stuck_topic, "garage/door/stuck");
    assert_eq!(config.door.travel_duration, embassy_time::Duration::from_millis(30_000));
    // omitted from the TOML above, so it falls back to the default
    assert_eq!(config.door.debounce_duration, embassy_time::Duration::from_millis(1_500));
    assert_eq!(config.door.open_sensor_topic, "garage/door/open_sensor");
    assert_eq!(config.door.closed_sensor_topic, "garage/door/closed_sensor");
    assert_eq!(config.door.safe_to_close_topic, "garage/door/safe_to_close");
    assert_eq!(config.door.max_attempts, 3);

    assert_eq!(
      config.door.remote.pressed_duration,
      embassy_time::Duration::from_millis(500)
    );
    assert_eq!(
      config.door.remote.wait_duration,
      embassy_time::Duration::from_millis(1_000)
    );
  }
}
