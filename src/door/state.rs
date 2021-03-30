use std::{fmt, str::FromStr};

use log::{debug, warn};
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

impl FromStr for TargetState {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "OPEN" => Ok(TargetState::Open),
      "CLOSED" => Ok(TargetState::Closed),
      _ => Err(()),
    }
  }
}

impl fmt::Display for TargetState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TargetState::Open => write!(f, "OPEN"),
      TargetState::Closed => write!(f, "CLOSED"),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stuck {
  Ok,
  Stuck,
}

impl fmt::Display for Stuck {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Stuck::Ok => write!(f, "ok"),
      Stuck::Stuck => write!(f, "stuck"),
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

impl fmt::Display for State {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      State::Opening => write!(f, "opening"),
      State::Open => write!(f, "open"),
      State::Closing => write!(f, "closing"),
      State::Closed => write!(f, "closed"),
    }
  }
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
      debug!("{} moving to state: {:?}", &self, &target_state);
      self.target_state = target_state;

      for _ in 0..MAX_STUCK_TRAVELS {
        match self.travel_if_needed().await? {
          TravelResult::Successful => {
            self.set_stuck(Stuck::Ok);
            return Ok(());
          }
          TravelResult::Failed => continue,
        }
      }

      warn!("Garage appears to be stuck!");
      self.set_stuck(Stuck::Stuck);
    }

    Ok(())
  }

  pub fn set_stuck(&mut self, stuck: Stuck) {
    self.stuck = stuck;
    if let Some(stuck_topic) = &self.stuck_topic {
      self
        .send_channel
        .send(MqttPublish {
          topic: stuck_topic.clone(),
          qos: QoS::AtLeastOnce,
          retain: true,
          payload: stuck.to_string(),
        })
        .expect("MQTT channel closed");
    }
  }

  pub async fn set_current_state(&mut self, current_state: State) -> GarageResult<()> {
    debug!("{} setting new state: {:?}", &self, current_state);
    self.current_state = current_state;
    self
      .send_channel
      .send(MqttPublish {
        topic: self.state_topic.clone(),
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: current_state.to_string(),
      })
      .expect("MQTT channel closed");
    Ok(())
  }

  async fn travel_if_needed(&mut self) -> GarageResult<TravelResult> {
    if self.current_state != self.target_state {
      // we're not in our target state, transition to travelling and trigger the door
      self.set_current_state(self.target_state.travel_state()).await?;

      // trigger the door
      debug!("{} triggering remote", &self);
      self.remote.trigger().await;

      self.monitor_travel().await
    }
    else {
      Ok(TravelResult::Successful)
    }
  }

  /// The door is moving, wait for it to move then observe the outcome
  async fn monitor_travel(&mut self) -> GarageResult<TravelResult> {
    // then wait for it to move
    debug!("{} travelling...", &self);
    let detected_state = self.state_detector.travel(self.target_state).await;
    debug!("{} travel result: {:?}", &self, &detected_state);


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
      debug!("{} state manually changed to: {:?}", &self, &detected_state);
      // door was closed but it's now open
      self.target_state = TargetState::Open;
      self.set_current_state(State::Opening).await?;
      self.monitor_travel().await?;
    }
    else if detected_state == DetectedState::Closed && self.current_state == State::Open {
      debug!("{} state manually changed to: {:?}", &self, &detected_state);
      // door was open but it's now closed
      self.target_state = TargetState::Closed;
      // we don't need to monitor travel because if it's close it's 100% closed
      self.set_current_state(State::Closed).await?;
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
