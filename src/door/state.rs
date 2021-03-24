use futures::{future::BoxFuture, FutureExt};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};

use super::{
  state_detector::{DetectedState, StateDetector},
  Door,
};
use crate::error::GarageResult;

/// The state the door is trying to get to
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
  #[serde(rename = "open")]
  Open,
  #[serde(rename = "closed")]
  Closed,
}

impl TargetState {
  /// Get the travel state used to travel *to* this state
  fn travel_state(&self) -> State {
    match self {
      TargetState::Open => State::Opening,
      TargetState::Closed => State::Closing,
    }
  }
}

impl PartialEq<TargetState> for State {
  fn eq(&self, other: &TargetState) -> bool {
    match (self, other) {
      (State::Open, TargetState::Open) | (State::Closed, TargetState::Closed) => true,
      _ => false,
    }
  }
}

#[derive(Serialize, Debug, Clone, Copy)]
pub enum State {
  #[serde(rename = "opening")]
  Opening,
  #[serde(rename = "open")]
  Open,
  #[serde(rename = "closing")]
  Closing,
  #[serde(rename = "closed")]
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

impl From<TargetState> for State {
  fn from(target_state: TargetState) -> Self {
    match target_state {
      TargetState::Open => State::Open,
      TargetState::Closed => State::Closed,
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

  /// Gets the target state this state started in before any transition
  pub fn start_state(&self) -> TargetState {
    match self {
      State::Opening | State::Closed => TargetState::Closed,
      State::Closing | State::Open => TargetState::Open,
    }
  }

  /// True if the state if opening or closing (i.e. in transition)
  pub fn is_travelling(&self) -> bool {
    match self {
      State::Opening | State::Closing => true,
      _ => false,
    }
  }
}

const MAX_STUCK_TRAVELS: usize = 5;

impl<'a, D: StateDetector + Send> Door<'a, D> {
  /// Tell the door to transition to the given target state
  pub async fn to_target_state(&mut self, target_state: TargetState) -> GarageResult<()> {
    // if this is already our target state we don't need to do anything
    if self.target_state != target_state {
      self.target_state = target_state;
      self.travel_if_needed(MAX_STUCK_TRAVELS).await?;
    }

    Ok(())
  }

  async fn set_current_state(&mut self, current_state: State) -> GarageResult<()> {
    self.current_state = current_state;
    self
      .mqtt_client
      .publish(
        &self.state_topic,
        QoS::AtLeastOnce,
        true,
        &toml::to_string(&current_state).unwrap(),
      )
      .await?;
    Ok(())
  }

  fn travel_if_needed<'b>(
    &'b mut self,
    remaining_travels: usize,
  ) -> Pin<Box<dyn Future<Output = GarageResult<()>> + 'b>> {
    Box::pin(async move {
      if self.current_state.is_travelling() {
        // if the door is currently travelling it'll check once it is finished
        // and then move if required, so we don't need to do anything here
      }
      else if remaining_travels == 0 {
        // the door appears to be stuck
        // TODO: door stuck alert
      }
      else if self.current_state != self.target_state {
        // we're not in our target state, transition to travelling and trigger the door
        self.set_current_state(self.target_state.travel_state()).await?;
        // trigger the door
        self.remote.trigger();
        // then wait for it to move
        let detected_state = self.state_detector.travel(self.target_state).await;

        // door (should have) finished moving, update our current state
        let (current_state, remaining_travels) = match detected_state {
          DetectedState::Open => (State::Open, remaining_travels),
          DetectedState::Closed => (State::Closed, remaining_travels),
          // if the door seems to be stuck we assume it is where it was when it opened and reduce the number of times we're willing to try again
          DetectedState::Stuck => (self.current_state.start_state().into(), remaining_travels - 1),
        };
        self.set_current_state(current_state).await?;

        // travel again if needed
        self.travel_if_needed(remaining_travels).await?;
      }

      Ok(())
    })
  }
}
