use rppal::gpio::{Gpio, InputPin, OutputPin};

pub use config::DoorConfig;
pub use identifier::Identifier;

pub mod config;
pub mod identifier;

#[derive(Debug)]
pub struct Door {
  identifier: Identifier,
  config: DoorConfig,
  remote_pin: OutputPin,
  sensor_pin: Option<InputPin>,
}

impl Door {
  pub fn with_config(identifier: Identifier, config: DoorConfig) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let remote_pin = gpio.get(config.remote.pin.pin_number())?.into_output();
    let sensor_pin = config
      .sensor
      .map(|sensor| gpio.get(sensor.pin.pin_number())?.into_input_pullup());

    Door {
      identifier,
      config,
      remote_pin,
      sensor_pin,
    }
  }
}
