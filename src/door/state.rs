use std::char::MAX;

use futures::{future::BoxFuture, FutureExt};
use rumqttc::QoS;
use serde::{Deserialize, Serialize};

use super::{
  state_detector::{DetectedState, StateDetector},
  Door,
};
use crate::{error::GarageResult, mqtt_client::MqttPublish};

/// The state the door is trying to get to
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
  #[serde(rename = "OPEN")]
  Open,
  #[serde(rename = "CLOSED")]
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

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
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

impl<D: StateDetector + Send> Door<D> {
  /// Tell the door to transition to the given target state
  pub async fn to_target_state(&mut self, target_state: TargetState) -> GarageResult<()> {
    if self.current_state.is_travelling() {
      panic!("attempted to set target state while door is travelling");
    }
    // if this is already our target state we don't need to do anything
    if self.target_state != target_state {
      println!("{} moving to state: {:?}", &self, &target_state);
      self.target_state = target_state;

      for _ in 0..MAX_STUCK_TRAVELS {
        match self.travel_if_needed(MAX_STUCK_TRAVELS).await? {
          TravelResult::Successful => return Ok(()),
          TravelResult::Failed => continue,
        }
      }

      println!("Warning! door move failed");
      // TODO: door moved failed
    }

    Ok(())
  }

  pub async fn set_current_state(&mut self, current_state: State) -> GarageResult<()> {
    self.current_state = current_state;
    self
      .send_channel
      .send(MqttPublish {
        topic: self.state_topic.clone(),
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: toml::to_string(&current_state).unwrap(),
      })
      .expect("MQTT channel cloesd");
    Ok(())
  }

  async fn travel_if_needed(&mut self, remaining_travels: usize) -> GarageResult<TravelResult> {
    if self.current_state != self.target_state {
      // we're not in our target state, transition to travelling and trigger the door
      self.set_current_state(self.target_state.travel_state()).await?;

      // trigger the door
      println!("{} triggering remote", &self);
      self.remote.trigger();

      self.monitor_travel().await
    }
    else {
      Ok(TravelResult::Successful)
    }
  }

  /// The door is moving, wait for it to move then observe the outcome
  async fn monitor_travel(&mut self) -> GarageResult<TravelResult> {
    // then wait for it to move
    println!("{} travelling...", &self);
    let detected_state = self.state_detector.travel(self.target_state).await;
    println!("{} travel result: {:?}", &self, &detected_state);


    // door (should have) finished moving, update our current state
    let (current_state, result) = match detected_state {
      DetectedState::Open => (State::Open, TravelResult::Successful),
      DetectedState::Closed => (State::Closed, TravelResult::Successful),
      // if the door seems to be stuck we assume it is where it was when it opened and reduce the number of times we're willing to try again
      DetectedState::Stuck => (self.current_state.start_state().into(), TravelResult::Failed),
    };
    self.set_current_state(current_state).await?;

    Ok(result)
  }

  /// Check the sensor's detected state, if different we assume the door was manually opened.
  /// Thus we invoke a travel (without triggering the door)
  pub async fn check_state(&mut self) -> GarageResult<()> {
    let detected_state = self.state_detector.detect_state();
    if detected_state == DetectedState::Open && self.current_state == State::Closed {
      println!("{} state manually changed to: {:?}", &self, &detected_state);
      // door was closed but it's now open
      self.target_state = TargetState::Open;
      self.monitor_travel().await?;
    }
    else if detected_state == DetectedState::Closed && self.current_state == State::Open {
      println!("{} state manually changed to: {:?}", &self, &detected_state);
      // door was open but it's now closed
      self.target_state = TargetState::Closed;
      self.monitor_travel().await?;
    }

    Ok(())
  }
}

enum TravelResult {
  /// We got to our target state
  Successful,
  /// The door
  Failed,
}
