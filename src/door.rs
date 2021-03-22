use self::{remote::DoorRemote, sensor::DoorSensor};
use crate::error::GarageResult;
pub use config::DoorConfig;
pub use identifier::Identifier;
use std::time::Duration;

pub mod config;
pub mod identifier;
mod remote;
mod sensor;

#[derive(Debug)]
pub struct Door {
  identifier: Identifier,
  topic_name: String,
  travel_time: Duration,
  remote: DoorRemote,
  sensor: Option<DoorSensor>,
}

impl Door {
  pub fn with_config(identifier: Identifier, config: DoorConfig) -> GarageResult<Self> {
    let remote = DoorRemote::with_config(config.remote)?;
    let sensor = config
      .sensor
      .map(|sensor_config| DoorSensor::with_config(sensor_config))
      .transpose()?;

    Ok(Door {
      identifier,
      topic_name: config.topic_name,
      travel_time: config.travel_time,
      remote,
      sensor,
    })
  }
}
