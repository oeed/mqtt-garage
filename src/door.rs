use rppal::gpio::{Gpio, InputPin, OutputPin};

pub use config::DoorConfig;
pub use identifier::Identifier;

pub mod config;
pub mod identifier;

#[derive(Debug)]
pub struct Door {
  identifier: Identifier,
  remote_pin: OutputPin,
  state_pin: Option<InputPin>,
}

impl Door {
  pub fn with_config(identifier: Identifier, config: DoorConfig) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let remote_pin = gpio.get(config.remote_pin.pin_number())?.into_output();
    let state_pin = config
      .state_pin
      .map(|state_pin| gpio.get(state_pin.pin_number())?.into_input_pullup());

    Door {
      identifier,
      remote_pin,
      state_pin,
    }
  }
}
