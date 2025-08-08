pub use config::RemoteConfig;
use embassy_time::{Instant, Timer};
use esp_idf_svc::hal::gpio::{self, Gpio14, PinDriver};
use smart_leds::colors;

use crate::{config::CONFIG, error::GarageResult, rgb::RgbLed};

pub struct DoorRemote<'a> {
  pin: PinDriver<'static, Gpio14, gpio::Output>,
  rgb_led: &'a mut RgbLed,
}

impl<'a> DoorRemote<'a> {
  pub fn new(gpio: Gpio14, rgb_led: &'a mut RgbLed) -> GarageResult<Self> {
    let mut pin = PinDriver::output(gpio)?;
    // Ensure the relay/output starts in a safe (off) state
    pin.set_low()?;

    Ok(DoorRemote { pin, rgb_led })
  }

  /// Trigger the remote to send the open/close signal
  pub async fn trigger(&mut self) -> GarageResult<()> {
    log::info!("Triggering remote");
    // NOTE: in future, if multiple doors/remotes are added, use a mutex when sending to prevent signal interference
    self.pin.set_high()?;
    self.rgb_led.on(colors::LIME);
    let t0 = Instant::now();
    Timer::after(CONFIG.door.remote.pressed_duration).await;
    log::info!("Remote press window elapsed ({} ms)", t0.elapsed().as_millis());
    log::info!("Releasing remote");
    self.pin.set_low()?;
    self.rgb_led.off();
    let t1 = Instant::now();
    Timer::after(CONFIG.door.remote.wait_duration).await;
    log::info!("Post-press settle elapsed ({} ms)", t1.elapsed().as_millis());
    Ok(())
  }
}
