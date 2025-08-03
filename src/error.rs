use thiserror::Error;

pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug, Error)]
pub enum GarageError {
  #[error(transparent)]
  EspError(#[from] esp_idf_svc::sys::EspError),
  // MqttConnection(#[from] rumqttc::ConnectionError),
  #[error("the MQTT client has been closed")]
  MqttClosed,
  // #[error(transparent)]
  // JoinError(#[from] JoinError),
  #[error("door initialisation timeout (sensor state not available)")]
  DoorInitialisationTimeout,
}
