use std::sync::Arc;

pub use config::RemoteConfig;
use mutex::RemoteMutex;
use rppal::gpio::{Gpio, OutputPin};

use crate::error::GarageResult;

mod config;
pub mod mutex;

#[derive(Debug)]
pub struct DoorRemote {
  pin: OutputPin,
  config: RemoteConfig,
  mutex: Arc<RemoteMutex>,
}

impl DoorRemote {
  pub fn with_config(config: RemoteConfig, mutex: Arc<RemoteMutex>) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let pin = gpio.get(config.pin.pin_number())?.into_output();

    Ok(DoorRemote { pin, config, mutex })
  }

  /// Trigger the remote to send the open/close signal
  pub async fn trigger(&mut self) {
    let guard = self.mutex.lock();
    self.pin.set_high();
    tokio::time::sleep(self.config.pressed_time).await;
    self.pin.set_low();
    tokio::time::sleep(self.config.wait_time).await;
  }
}
