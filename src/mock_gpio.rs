//! Mimics rppal's API without the need to compile to ARM and use physical hardware

pub use std::fmt::Error;
use std::fs;

pub struct Gpio;

impl Gpio {
  pub fn new() -> Result<Gpio, Error> {
    Ok(Gpio)
  }

  pub fn get(&self, pin: u8) -> Result<Pin, Error> {
    Ok(Pin(pin))
  }
}

#[derive(Debug)]
pub struct Pin(u8);

impl Pin {
  pub fn into_output(self) -> OutputPin {
    OutputPin(self.0)
  }

  pub fn into_input_pullup(self) -> InputPin {
    InputPin(self.0)
  }
}

#[derive(Debug)]
pub struct OutputPin(u8);

impl OutputPin {
  pub fn set_high(&self) {
    println!("{:?} set to high", self)
  }

  pub fn set_low(&self) {
    println!("{:?} set to low", self)
  }
}


#[derive(Debug)]
pub struct InputPin(u8);

impl InputPin {
  pub fn is_high(&self) -> bool {
    fs::read_to_string(format!("{}.pin", self.0))
      .ok()
      .map(|value| value == "1")
      .unwrap_or(false)
  }
}
