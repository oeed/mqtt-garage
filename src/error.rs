use thiserror::Error;
use tokio::task::JoinError;

use crate::door::identifier::Identifier;
pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug, Error)]
pub enum GarageError {
  #[error(transparent)]
  #[cfg(feature = "arm")]
  GPIO(#[from] rppal::gpio::Error),
  #[cfg(not(feature = "arm"))]
  #[error(transparent)]
  GPIO(#[from] crate::mock_gpio::Error),
  #[error(transparent)]
  MQTTClient(#[from] rumqttc::ClientError),
  #[error(transparent)]
  MQTTConnection(#[from] rumqttc::ConnectionError),
  #[error(transparent)]
  JoinError(#[from] JoinError),
  #[error("door initialisation timeout for {0:?}")]
  DoorInitialisationTimeout(Identifier),
}
