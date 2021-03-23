use super::{mqtt::MqttConfig, remote::RemoteConfig, state_detector::StateDetectorConfig};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DoorConfig {
  /// The name of the MQTT topic to push to
  pub mqtt: MqttConfig,

  /// The door detector sensor (if available)
  pub state_detector: StateDetectorConfig,

  /// The remote used to open and close the door
  pub remote: RemoteConfig,
}
