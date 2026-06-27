use thiserror::Error;

pub type GarageResult<T> = Result<T, GarageError>;

#[derive(Debug, Error)]
pub enum GarageError {
  #[error(transparent)]
  EspError(#[from] esp_idf_svc::sys::EspError),
  #[error(transparent)]
  Ws2812Error(#[from] ws2812_esp32_rmt_driver::Ws2812Esp32RmtDriverError),
  #[error("the MQTT client has been closed")]
  MqttClosed,
  #[error("door initialisation timeout (sensor state not available)")]
  DoorInitialisationTimeout,
  #[error("wifi disconnected")]
  WifiDisconnected,
}
