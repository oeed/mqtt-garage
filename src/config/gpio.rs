use serde::Deserialize;

/// Mapping of GPIO pin names to their actual pin number
/// See: https://pinout.xyz/
#[derive(Debug, Deserialize)]
pub enum GpioPin {
  Gpio2,
  Gpio3,
  Gpio4,
  Gpio17,
  Gpio27,
  Gpio22,
  Gpio10,
  Gpio9,
  Gpio11,
  Gpio0,
  Gpio5,
  Gpio6,
  Gpio13,
  Gpio19,
  Gpio26,
  Gpio14,
  Gpio15,
  Gpio18,
  Gpio23,
  Gpio24,
  Gpio25,
  Gpio8,
  Gpio7,
  Gpio1,
  Gpio12,
  Gpio16,
  Gpio20,
  Gpio21,
}

impl GpioPin {
  /// Get the pin number for this GPIO pin
  pub fn board_number(&self) -> u8 {
    use GpioPin::*;

    match self {
      Gpio2 => 3,
      Gpio3 => 5,
      Gpio4 => 6,
      Gpio17 => 11,
      Gpio27 => 13,
      Gpio22 => 15,
      Gpio10 => 19,
      Gpio9 => 21,
      Gpio11 => 23,
      Gpio0 => 27,
      Gpio5 => 29,
      Gpio6 => 31,
      Gpio13 => 33,
      Gpio19 => 35,
      Gpio26 => 26,
      Gpio14 => 8,
      Gpio15 => 10,
      Gpio18 => 12,
      Gpio23 => 16,
      Gpio24 => 18,
      Gpio25 => 22,
      Gpio8 => 24,
      Gpio7 => 37,
      Gpio1 => 28,
      Gpio12 => 32,
      Gpio16 => 36,
      Gpio20 => 38,
      Gpio21 => 40,
    }
  }

  /// Get the pin number for this GPIO pin
  pub fn bcm_number(&self) -> u8 {
    use GpioPin::*;

    match self {
      Gpio2 => 2,
      Gpio3 => 3,
      Gpio4 => 4,
      Gpio17 => 17,
      Gpio27 => 27,
      Gpio22 => 22,
      Gpio10 => 10,
      Gpio9 => 9,
      Gpio11 => 11,
      Gpio0 => 0,
      Gpio5 => 5,
      Gpio6 => 6,
      Gpio13 => 13,
      Gpio19 => 19,
      Gpio26 => 26,
      Gpio14 => 14,
      Gpio15 => 15,
      Gpio18 => 18,
      Gpio23 => 23,
      Gpio24 => 24,
      Gpio25 => 25,
      Gpio8 => 8,
      Gpio7 => 7,
      Gpio1 => 1,
      Gpio12 => 12,
      Gpio16 => 16,
      Gpio20 => 20,
      Gpio21 => 21,
    }
  }
}
