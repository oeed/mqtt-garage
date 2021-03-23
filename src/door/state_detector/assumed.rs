use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};

use super::{DetectedState, StateDetector, Travel};
use crate::{
  door::{state::TargetState, Identifier},
  error::GarageResult,
};

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct AssumedStateDetectorConfig {
  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is assumed to take to go to/from open/close.
  pub travel_time: Duration,
}


#[derive(Debug)]
pub struct AssumedStateDetector {
  identifier: Identifier, // TODO: can we borrow this?
  travel_time: Duration,
  current_travel: Option<Travel>,
  assumed_state: DetectedState,
}

#[async_trait]
impl StateDetector for AssumedStateDetector {
  type Config = AssumedStateDetectorConfig;

  fn with_config(identifier: Identifier, config: Self::Config) -> GarageResult<Self> {
    Ok(AssumedStateDetector {
      identifier,
      travel_time: config.travel_time,
      current_travel: None,
      assumed_state: DetectedState::Closed, // TODO: write to file
    })
  }

  async fn travel(&mut self, target_state: TargetState) -> DetectedState {
    if self.current_travel.is_some() {
      panic!("AssumedStateDetector attempted to travel while it was already travelling");
    }
    self.current_travel = Some(Travel::new(target_state));
    tokio::time::sleep(self.travel_time).await;
    self.detect_state()
  }

  fn detect_state(&mut self) -> DetectedState {
    if let Some(current_travel) = &self.current_travel {
      if current_travel.expired_invalid(self.assumed_state, self.travel_time) {
        // door was moving and should've finished by now, we assume it's finished. move to the target state
        self.assumed_state = DetectedState::from(current_travel.target_state);
        self.current_travel = None;
        // TODO: write to file
        self.assumed_state
      }
      else {
        // door is moving, use the previous state (which is the inverse of the target state)
        self.assumed_state
      }
    }
    else {
      // the door isn't moving, so we use the previously assumed state
      self.assumed_state
    }
  }
}
