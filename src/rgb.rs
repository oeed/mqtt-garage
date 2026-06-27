use esp_idf_svc::hal::{gpio::Gpio48, rmt};
use smart_leds::{RGB, SmartLedsWrite};
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use crate::error::GarageResult;

pub struct RgbLed {
  ws2812: Ws2812Esp32Rmt<'static>,
}

impl RgbLed {
  pub fn new(channel: rmt::CHANNEL0, gpio: Gpio48) -> GarageResult<RgbLed> {
    let ws2812 = Ws2812Esp32Rmt::new(channel, gpio)?;
    Ok(RgbLed { ws2812 })
  }

  pub fn on(&mut self, rgb: RGB<u8>) {
    let dimmer = RGB::new(rgb.r / 20, rgb.g / 20, rgb.b / 20);
    self.ws2812.write(std::iter::once(dimmer)).unwrap();
  }

  pub fn off(&mut self) {
    self.ws2812.write(std::iter::once(RGB::default())).unwrap();
  }
}
