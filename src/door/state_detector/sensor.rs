use crate::{config::gpio::GpioPin, error::GarageResult};
use rppal::gpio::{Gpio, InputPin};
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::time::Duration;

use super::StateDetector;

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SensorStateDetectorConfig {
  /// The pin of the door detector sensor (if available)
  pub pin: GpioPin,

  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is expected to take to go to/from open/close.
  ///
  /// If it exceeds this it tries again.
  pub travel_time: Duration,
}


#[derive(Debug)]
pub struct SensorStateDetector {
  pin: InputPin,
  travel_time: Duration,
}

impl StateDetector for SensorStateDetector {
  type Config = SensorStateDetectorConfig;

  fn with_config(config: Self::Config) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let pin = gpio.get(config.pin.pin_number())?.into_input_pullup();

    Ok(SensorStateDetector {
      pin,
      travel_time: config.travel_time,
    })
  }
}
