use serde::Deserialize;

use super::{controller::config::DoorControllerConfig, detector::DoorDetector};


#[derive(Debug, Deserialize)]
pub struct DoorConfig<D: DoorDetector> {
  pub detector: D::Config,
  pub controller: DoorControllerConfig,
}
