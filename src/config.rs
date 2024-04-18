use std::collections::HashMap;

use serde::Deserialize;

use crate::{
  door::{self, detector::AnyDoorDetector},
  mqtt_client::MqttClientConfig,
};

pub mod gpio;

#[derive(Debug, Deserialize)]
pub struct Config {
  /// The MQTT configuration
  pub mqtt_client: MqttClientConfig,
  /// A list of all doors to control
  pub doors: HashMap<String, door::config::DoorConfig<AnyDoorDetector>>,
}
