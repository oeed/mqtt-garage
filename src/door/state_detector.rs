use self::{assumed::AssumedStateDetectorConfig, sensor::SensorStateDetectorConfig};
use crate::error::GarageResult;
use serde::Deserialize;
use std::fmt::Debug;

pub mod assumed;
pub mod sensor;

pub trait StateDetector: Debug {
  type Config;

  fn with_config(config: Self::Config) -> GarageResult<Self>
  where
    Self: Sized;
}

#[serde(untagged)]
#[derive(Debug, Deserialize)]
pub enum StateDetectorConfig {
  Sensor(SensorStateDetectorConfig),
  Assumed(AssumedStateDetectorConfig),
}
