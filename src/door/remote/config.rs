use crate::config::gpio::GpioPin;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::time::Duration;

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct RemoteConfig {
  /// The pin of the door remote
  pub pin: GpioPin,

  #[serde_as(as = "DurationSeconds<f64>")]
  /// How long the remote pin is high for (i.e. how long the remote signal is sent)
  pub pressed_time: Duration,

  #[serde_as(as = "DurationSeconds<f64>")]
  /// How long to wait after pressing the remote before pressing another remote
  pub wait_time: Duration,
}
