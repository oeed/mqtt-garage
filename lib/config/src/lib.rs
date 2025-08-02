use std::borrow::Cow;

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
  pub ssid: Cow<'static, str>,
  pub password: Cow<'static, str>,
}


#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct MqttConfig {
  pub broker_domain: Cow<'static, str>,
  pub broker_port: u16,
  pub client_id: Cow<'static, str>,
  pub availability_topic: Cow<'static, str>,
  pub online_availability: Cow<'static, str>,
  pub offline_availability: Cow<'static, str>,
}


#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct RemoteConfig {
  pub pin: Cow<'static, str>,
  pub pressed_time: f32,
  pub wait_time: f32,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct DoorConfig {
  pub controller: ControllerConfig,
  pub detector: DetectorConfig,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct ControllerConfig {
  pub command_topic: Cow<'static, str>,
  pub initial_target_state: Cow<'static, str>,
  pub state_topic: Cow<'static, str>,
  pub stuck_topic: Cow<'static, str>,
  pub travel_duration: u64,
  pub remote: RemoteConfig,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct DetectorConfig {
  pub sensor_topic: Cow<'static, str>,
}
