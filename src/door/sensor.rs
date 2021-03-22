use crate::error::GarageResult;
use rppal::gpio::{Gpio, InputPin};

use super::config::SensorConfig;

#[derive(Debug)]
pub struct DoorSensor {
  config: SensorConfig,
  pin: InputPin,
}

impl DoorSensor {
  pub fn with_config(config: SensorConfig) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let pin = gpio.get(config.pin.pin_number())?.into_input_pullup();

    Ok(DoorSensor { config, pin })
  }
}
