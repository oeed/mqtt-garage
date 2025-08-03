pub use config::RemoteConfig;
use embassy_time::Timer;

use crate::{config::CONFIG, error::GarageResult};

#[derive(Debug)]
pub struct DoorRemote {
  // pin: OutputPin,
}

impl DoorRemote {
  pub fn new() -> GarageResult<Self> {
    // let gpio = Gpio::new()?;
    // let pin = gpio.get(config.pin.bcm_number())?.into_output();

    Ok(DoorRemote { /* pin */ })
  }

  /// Trigger the remote to send the open/close signal
  pub async fn trigger(&mut self) {
    // NOTE: in future, if multiple doors/remotes are added, use a mutex when sending to prevent signal interference
    // self.pin.set_high();
    Timer::after(CONFIG.door.remote.pressed_duration).await;
    // self.pin.set_low();
    Timer::after(CONFIG.door.remote.wait_duration).await;
  }
}
