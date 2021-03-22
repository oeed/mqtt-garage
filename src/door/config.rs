use crate::config::gpio::GpioPin;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::time::Duration;

#[serde_as]
#[derive(Deserialize)]
pub struct DoorConfig {
  /// The name of the MQTT topic to push to
  pub topic_name: String,
  /// The pin of the door detector sensor (if available)
  pub state_pin: Option<GpioPin>,
  /// The pin of the door remote
  pub remote_pin: GpioPin,
  #[serde_as(as = "DurationSeconds<u64>")]
  pub travel_time: Duration,
}
