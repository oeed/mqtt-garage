use std::sync::Arc;

pub use config::RemoteConfig;
use log::debug;
use mutex::RemoteMutex;
#[cfg(feature = "arm")]
use rppal::gpio::{Gpio, OutputPin};

use crate::error::GarageResult;
#[cfg(not(feature = "arm"))]
use crate::mock_gpio::{Gpio, OutputPin};

mod config;
pub mod mutex;

#[derive(Debug)]
pub struct DoorRemote {
  pin: OutputPin,
  pub config: RemoteConfig,
  mutex: Arc<RemoteMutex>,
}

impl DoorRemote {
  pub fn new(config: RemoteConfig, mutex: Arc<RemoteMutex>) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let pin = gpio.get(config.pin.bcm_number())?.into_output();

    Ok(DoorRemote { pin, config, mutex })
  }

  /// Trigger the remote to send the open/close signal
  pub async fn trigger(&mut self) {
    let guard = self.mutex.lock().await;
    debug!("Locked remote mutex");
    self.pin.set_high();
    tokio::time::sleep(self.config.pressed_time).await;
    self.pin.set_low();
    tokio::time::sleep(self.config.wait_time).await;
    debug!("Unlocked remote mutex");
    drop(guard);
  }
}
