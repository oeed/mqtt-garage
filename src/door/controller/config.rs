use std::time::Duration;

use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};

use super::remote::RemoteConfig;
use crate::door::state::TargetState;


#[serde_as]
#[derive(Debug, Deserialize)]
pub struct DoorControllerConfig {
  /// The name of the MQTT topic open/close commands are received on
  pub command_topic: String,

  /// The name of the MQTT topic state change commands are sent on
  pub state_topic: String,

  /// The name of the MQTT topic stuck state change commands are sent on, if desired
  pub stuck_topic: Option<String>,

  /// If set, when first turned on the door will attempt to move to this state
  pub initial_target_state: Option<TargetState>,

  /// The remote used to open and close the door
  pub remote: RemoteConfig,

  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is expected to take to go to/from open/close.
  ///
  /// If it exceeds this it tries again.
  pub travel_duration: Duration,

  #[serde_as(as = "DurationSeconds<u64>")]
  /// The maximum time the remote's signal can take to start moving the door.
  ///
  /// If the door doesn't open after this time it'll try again.
  pub max_remote_latency_duration: Duration,
}
