pub use config::RemoteConfig;
use embassy_time::Timer;
use esp_idf_svc::hal::gpio::{self, Gpio16, PinDriver, Pins};

use crate::{config::CONFIG, error::GarageResult};

pub struct DoorRemote {
  pin: PinDriver<'static, Gpio16, gpio::Output>,
}

impl DoorRemote {
  pub fn new(pins: Pins) -> GarageResult<Self> {
    let pin = PinDriver::output(pins.gpio16)?;

    Ok(DoorRemote { pin })
  }

  /// Trigger the remote to send the open/close signal
  pub async fn trigger(&mut self) -> GarageResult<()> {
    log::info!("Triggering remote");
    // NOTE: in future, if multiple doors/remotes are added, use a mutex when sending to prevent signal interference
    self.pin.set_high()?;
    Timer::after(CONFIG.door.remote.pressed_duration).await;
    self.pin.set_low()?;
    Timer::after(CONFIG.door.remote.wait_duration).await;
    Ok(())
  }
}
