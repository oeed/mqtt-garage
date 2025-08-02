#[cfg(feature = "serde")]
use serde::Deserialize;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct Config {
  pub wifi: WifiConfig,
  pub mqtt: MqttConfig,
  pub door: DoorConfig,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct WifiConfig {
  pub ssid: &'static str,
  pub password: &'static str,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct MqttConfig {
  pub broker_domain: &'static str,
  pub broker_port: u16,
  pub client_id: &'static str,
  pub availability_topic: &'static str,
  pub online_availability: &'static str,
  pub offline_availability: &'static str,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct RemoteConfig {
  pub pin: &'static str,
  pub pressed_time: f32,
  pub wait_time: f32,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct DetectorConfig {
  pub sensor_topic: &'static str,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub struct ControllerConfig {
  pub command_topic: &'static str,
  pub initial_target_state: &'static str,
  pub state_topic: &'static str,
  pub stuck_topic: &'static str,
  pub travel_duration: u64,
  pub remote: RemoteConfig,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct DoorConfig {
  pub controller: ControllerConfig,
  pub detector: DetectorConfig,
}
