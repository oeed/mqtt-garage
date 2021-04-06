use std::{error, fmt};

use tokio::task::JoinError;

pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug)]
pub enum GarageError {
  #[cfg(feature = "arm")]
  GPIO(rppal::gpio::Error),
  #[cfg(not(feature = "arm"))]
  GPIO(crate::mock_gpio::Error),
  MQTTClient(rumqttc::ClientError),
  MQTTConnection(rumqttc::ConnectionError),
  JoinError(JoinError),
}


impl fmt::Display for GarageError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match *self {
      GarageError::GPIO(ref e) => e.fmt(f),
      GarageError::MQTTClient(ref e) => e.fmt(f),
      GarageError::MQTTConnection(ref e) => e.fmt(f),
      GarageError::JoinError(ref e) => e.fmt(f),
    }
  }
}

impl error::Error for GarageError {
  fn source(&self) -> Option<&(dyn error::Error + 'static)> {
    match *self {
      GarageError::GPIO(ref e) => Some(e),
      GarageError::MQTTClient(ref e) => Some(e),
      GarageError::MQTTConnection(ref e) => Some(e),
      GarageError::JoinError(ref e) => Some(e),
    }
  }
}

#[cfg(feature = "arm")]
impl From<rppal::gpio::Error> for GarageError {
  fn from(err: rppal::gpio::Error) -> GarageError {
    GarageError::GPIO(err)
  }
}

#[cfg(not(feature = "arm"))]
impl From<crate::mock_gpio::Error> for GarageError {
  fn from(err: crate::mock_gpio::Error) -> GarageError {
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

impl From<JoinError> for GarageError {
  fn from(err: JoinError) -> GarageError {
    GarageError::JoinError(err)
  }
}
