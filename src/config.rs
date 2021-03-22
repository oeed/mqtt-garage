use crate::door;
use serde::Deserialize;
use std::collections::HashMap;

pub mod gpio;

#[derive(Deserialize)]
struct Config {
  /// The domain of the MQTT broker
  mqtt_broker: String,
  /// A list of all doors to control
  doors: HashMap<door::Identifier, door::DoorConfig>,
}
