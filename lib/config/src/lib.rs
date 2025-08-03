use std::borrow::Cow;

use serde::{Deserialize, Deserializer};
#[derive(Debug, Deserialize)]
pub struct Config {
  pub wifi: WifiConfig,
  pub mqtt: MqttConfig,
  pub door: DoorConfig,
}


#[derive(Debug, Deserialize)]
pub struct WifiConfig {
  pub ssid: Cow<'static, str>,
  pub password: Cow<'static, str>,
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
  pub pin: Cow<'static, str>,
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
  pub sensor_topic: Cow<'static, str>,
  pub max_attempts: u8,
}


fn deserialize_duration_millis<'de, D>(deserializer: D) -> Result<embassy_time::Duration, D::Error>
where
  D: Deserializer<'de>,
{
  let millis: u64 = Deserialize::deserialize(deserializer)?;
  Ok(embassy_time::Duration::from_millis(millis))
}


#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_deserialize_config() {
    let config_str = r#"
[wifi]
ssid = "my-ssid"
password = "my-password"

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
travel_duration = 30.0
sensor_topic = "garage/door/sensor"
max_attempts = 3

[door.remote]
pin = "1"
pressed_time = 0.5
wait_time = 1.0
"#;

    let config: Config = toml::from_str(config_str).unwrap();

    assert_eq!(config.wifi.ssid, "my-ssid");
    assert_eq!(config.wifi.password, "my-password");

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
    assert_eq!(config.door.sensor_topic, "garage/door/sensor");
    assert_eq!(config.door.max_attempts, 3);

    assert_eq!(config.door.remote.pin, "1");
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
