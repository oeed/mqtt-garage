use serde::Deserialize;

use super::{
  config::MqttConfig,
  state_detector::{DetectedState, StateDetector},
  Door,
};
use crate::{error::GarageResult, mqtt_client::MqttClient};

/// The state the door is trying to get to
#[derive(Deserialize, Debug, Clone, Copy)]
pub enum TargetState {
  Open,
  Closed,
}

#[derive(Debug)]
pub enum State {
  Opening,
  Open,
  Closing,
  Closed,
}

impl From<DetectedState> for State {
  fn from(target_state: DetectedState) -> Self {
    match target_state {
      DetectedState::Open => State::Open,
      DetectedState::Closed => State::Closed,
      DetectedState::Stuck => State::Open,
    }
  }
}

impl State {
  /// Gets the target state this state will end up in (or is currently in)
  pub fn end_state(&self) -> TargetState {
    match self {
      State::Opening | State::Open => TargetState::Open,
      State::Closing | State::Closed => TargetState::Closed,
    }
  }
}

impl<'a, D: StateDetector> Door<'a, D> {
  pub async fn to_target_state(&mut self, target_state: TargetState) -> GarageResult<()> {
    Ok(())
  }
}
