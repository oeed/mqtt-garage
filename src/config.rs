#[cfg(feature = "arm")]
use std::collections::HashMap;

use serde::Deserialize;

#[cfg(feature = "arm")]
use crate::door;
use crate::mqtt_client::MqttClientConfig;

pub mod gpio;

#[derive(Debug, Deserialize)]
pub struct Config {
  /// The MQTT configuration
  pub mqtt_client: MqttClientConfig,
  /// A list of all doors to control
  #[cfg(feature = "arm")]
  pub doors: HashMap<door::Identifier, door::DoorConfig>,
}
