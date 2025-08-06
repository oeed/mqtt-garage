pub use config::RemoteConfig;
use embassy_time::Timer;
use esp_idf_svc::hal::gpio::{self, Gpio14, PinDriver};
use smart_leds::colors;

use crate::{config::CONFIG, error::GarageResult, rgb::RgbLed};

pub struct DoorRemote<'a> {
  pin: PinDriver<'static, Gpio14, gpio::Output>,
  rgb_led: &'a mut RgbLed,
}

impl<'a> DoorRemote<'a> {
  pub fn new(gpio: Gpio14, rgb_led: &'a mut RgbLed) -> GarageResult<Self> {
    let pin = PinDriver::output(gpio)?;

    Ok(DoorRemote { pin, rgb_led })
  }

  /// Trigger the remote to send the open/close signal
  pub async fn trigger(&mut self) -> GarageResult<()> {
    log::info!("Triggering remote");
    // NOTE: in future, if multiple doors/remotes are added, use a mutex when sending to prevent signal interference
    self.pin.set_high()?;
    self.rgb_led.on(colors::LIME);
    Timer::after(CONFIG.door.remote.pressed_duration).await;
    self.pin.set_low()?;
    self.rgb_led.off();
    Timer::after(CONFIG.door.remote.wait_duration).await;
    Ok(())
  }
}
