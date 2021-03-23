use serde::Deserialize;

use super::{remote::RemoteConfig, state::TargetState, state_detector::StateDetectorConfig};

#[derive(Debug, Deserialize)]
pub struct DoorConfig {
  /// The name of the MQTT topic to push to
  pub mqtt: MqttConfig,

  /// If set, when first turned on the door will attempt to move to this state
  pub initial_target_state: Option<TargetState>,

  /// The door detector sensor (if available)
  pub state_detector: StateDetectorConfig,

  /// The remote used to open and close the door
  pub remote: RemoteConfig,
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
  /// The name of the MQTT topic open/close commands are received on
  pub command_topic: String,

  /// The name of the MQTT topic state change commands are sent on
  pub state_topic: String,
}
