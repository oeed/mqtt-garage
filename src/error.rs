use std::{error, fmt};

pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug)]
pub enum GarageError {
  GPIO(rppal::gpio::Error),
}


impl fmt::Display for GarageError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match *self {
      GarageError::GPIO(ref e) => e.fmt(f),
    }
  }
}

impl error::Error for GarageError {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
    match *self {
      GarageError::GPIO(ref e) => Some(e),
    }
  }
}

impl From<rppal::gpio::Error> for GarageError {
  fn from(err: rppal::gpio::Error) -> GarageError {
    GarageError::GPIO(err)
  }
}
