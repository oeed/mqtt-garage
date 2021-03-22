pub type GarageResult<T> = Result<T, GarageError>;
pub enum GarageError {
  InvalidConfig,
  GPIO(rppal::gpio::Error),
}
