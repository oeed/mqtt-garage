use std::{error, fmt};

pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug)]
pub enum GarageError {
  #[cfg(feature = "arm")]
  GPIO(rppal::gpio::Error),
  MQTTClient(rumqttc::ClientError),
  MQTTConnection(rumqttc::ConnectionError),
}


impl fmt::Display for GarageError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match *self {
      #[cfg(feature = "arm")]
      GarageError::GPIO(ref e) => e.fmt(f),
      GarageError::MQTTClient(ref e) => e.fmt(f),
      GarageError::MQTTConnection(ref e) => e.fmt(f),
    }
  }
}

impl error::Error for GarageError {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
    match *self {
      #[cfg(feature = "arm")]
      GarageError::GPIO(ref e) => Some(e),
      GarageError::MQTTClient(ref e) => Some(e),
      GarageError::MQTTConnection(ref e) => Some(e),
    }
  }
}

#[cfg(feature = "arm")]
impl From<rppal::gpio::Error> for GarageError {
  fn from(err: rppal::gpio::Error) -> GarageError {
    GarageError::GPIO(err)
  }
}

impl From<rumqttc::ClientError> for GarageError {
  fn from(err: rumqttc::ClientError) -> GarageError {
    GarageError::MQTTClient(err)
  }
}


impl From<rumqttc::ConnectionError> for GarageError {
  fn from(err: rumqttc::ConnectionError) -> GarageError {
    GarageError::MQTTConnection(err)
  }
}
