#[cfg(feature = "serde")]
use serde::Deserialize;

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct Config<'a> {
  pub ssid: &'a str,
  pub wifi: WifiConfig<'a>,
  pub mqtt: MqttConfig<'a>,
  pub door: DoorConfig<'a>,
}


#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct WifiConfig<'a> {
  pub ssid: &'a str,
  pub password: &'a str,
}


#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct MqttConfig<'a> {
  pub broker_domain: &'a str,
  pub broker_port: u16,
  pub client_id: &'a str,
  pub availability_topic: &'a str,
  pub online_availability: &'a str,
  pub offline_availability: &'a str,
}


#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct RemoteConfig<'a> {
  pub pin: &'a str,
  pub pressed_time: f32,
  pub wait_time: f32,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(deserialize = "'de: 'a")))]
pub struct DoorConfig<'a> {
  pub controller: ControllerConfig<'a>,
  pub detector: DetectorConfig<'a>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct ControllerConfig<'a> {
  pub command_topic: &'a str,
  pub initial_target_state: &'a str,
  pub state_topic: &'a str,
  pub stuck_topic: &'a str,
  pub travel_duration: u64,
  pub remote: RemoteConfig<'a>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Deserialize))]
pub struct DetectorConfig<'a> {
  pub sensor_topic: &'a str,
}
