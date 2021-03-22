use crate::error::GarageResult;
use rppal::gpio::{Gpio, OutputPin};
use std::thread;

pub use config::RemoteConfig;

mod config;

#[derive(Debug)]
pub struct DoorRemote {
  pin: OutputPin,
  config: RemoteConfig,
}

impl DoorRemote {
  pub fn with_config(config: RemoteConfig) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let pin = gpio.get(config.pin.pin_number())?.into_output();

    Ok(DoorRemote { pin, config })
  }

  /// Trigger the remote to send the open/close signal
  pub fn trigger(&mut self) {
    // TOOD: make this async and deoverlap?
    self.pin.set_high();
    thread::sleep(self.config.pressed_time);
    self.pin.set_low();
    thread::sleep(self.config.wait_time);
  }
}
