use super::StateDetector;
use crate::error::GarageResult;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::time::Duration;

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct AssumedStateDetectorConfig {
  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is assumed to take to go to/from open/close.
  pub travel_time: Duration,
}


#[derive(Debug)]
pub struct AssumedStateDetector {
  travel_time: Duration,
}

impl StateDetector for AssumedStateDetector {
  type Config = AssumedStateDetectorConfig;

  fn with_config(config: Self::Config) -> GarageResult<Self> {
    Ok(AssumedStateDetector {
      travel_time: config.travel_time,
    })
  }
}
