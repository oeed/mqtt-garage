use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Deserialize;
use std::{error::Error, time::Duration};
use tokio::{task, time};

use crate::error::GarageResult;

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
  /// The name of the MQTT topic open/close commands are received on
  pub command_topic: String,
  /// The payload of the command to open the door
  pub open_command: String,
  /// The payload of the command to close the door
  pub close_command: String,

  /// The name of the MQTT topic state change commands are sent on
  pub state_topic: String,
  /// The payload of the state indicating the door is open
  pub open_state: String,
  /// The payload of the state indicating the door is opening
  pub opening_state: String,
  /// The payload of the state indicating the door is closed
  pub closed_state: String,
  /// The payload of the state indicating the door is closing
  pub closing_state: String,
}


#[derive(Debug)]
pub struct DoorMqtt {}

impl DoorMqtt {
  pub fn with_config(config: MqttConfig) -> GarageResult<Self> {
    Ok(DoorMqtt {})
  }
}
