use thiserror::Error;
use tokio::task::JoinError;

use crate::door::identifier::Identifier;
pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug, Error)]
pub enum GarageError {
  #[error(transparent)]
  #[cfg(feature = "arm")]
  Gpio(#[from] rppal::gpio::Error),
  #[cfg(not(feature = "arm"))]
  #[error(transparent)]
  Gpio(#[from] crate::mock_gpio::Error),
  #[error(transparent)]
  MqttClient(#[from] rumqttc::ClientError),
  #[error(transparent)]
  MqttConnection(#[from] rumqttc::ConnectionError),
  #[error("the MQTT client has been closed")]
  MqttClosed,
  #[error(transparent)]
  JoinError(#[from] JoinError),
  #[error("door initialisation timeout for {0:?}")]
  DoorInitialisationTimeout(Identifier),
}
