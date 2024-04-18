use std::{
  fmt::Debug,
  future::Future,
  time::{Duration, SystemTime},
};

use serde::Deserialize;

use self::{
  assumed::AssumedStateDetectorConfig, gpio::GpioStateDetectorConfig, zigbee2mqtt::Zigbee2MqttStateDetectorConfig,
};
use super::{
  state::{State, TargetState},
  Identifier,
};
use crate::{
  error::GarageResult,
  mqtt_client::{
    receiver::{MqttReceiver, PublishReceiver},
    MqttPublish,
  },
};

pub mod assumed;
pub mod gpio;
pub mod zigbee2mqtt;

pub trait StateDetector: Debug {
  type Config;

  fn with_config(identifier: Identifier, config: Self::Config) -> GarageResult<Self>
  where
    Self: Sized;

  /// Request the state the detector thinks the door is in.
  fn detect_state(&mut self) -> DetectedState;

  /// Invoked when the door starts moving to the target state.
  /// Used to track how long the door has been moving.
  ///
  /// Future resolves when the door *should* have finished travelling in the state it was detected in
  fn travel(&mut self, target_state: TargetState) -> impl Future<Output = DetectedState> + Send;

  /// whether the state detector should be periodically checked for updates
  fn should_check_periodically(&self) -> bool;

  fn subscribe(
    &mut self,
    _mqtt_receiver: &mut MqttReceiver,
  ) -> impl Future<Output = GarageResult<Option<PublishReceiver>>> + Send {
    async { Ok(None) }
  }

  fn receive_message(&mut self, _publish: MqttPublish) {}

  /// Get the state the door should be in when detecting a manual change to the given target state
  fn manual_travel_state(&self, target_state: TargetState) -> State;
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StateDetectorConfig {
  Gpio(GpioStateDetectorConfig),
  Assumed(AssumedStateDetectorConfig),
  Zigbee2Mqtt(Zigbee2MqttStateDetectorConfig),
}

/// Detectors can tell if a door is open or closed, but not where long it is.
///
/// It can also determine if the door is likely stuck.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DetectedState {
  Open,
  Closed,
  Stuck,
}

impl From<TargetState> for DetectedState {
  fn from(target_state: TargetState) -> Self {
    match target_state {
      TargetState::Open => DetectedState::Open,
      TargetState::Closed => DetectedState::Closed,
    }
  }
}

/// Represents a currently occuring door travel
#[derive(Debug)]
struct Travel {
  start_time: SystemTime,
  pub target_state: TargetState,
}

impl Travel {
  pub fn new(target_state: TargetState) -> Self {
    Travel {
      start_time: SystemTime::now(),
      target_state,
    }
  }

  /// True if the given travel has been occuring longer than the given duration and is in the wrong state
  pub fn expired_invalid(&self, detected_state: DetectedState, duration: Duration) -> bool {
    if detected_state != DetectedState::from(self.target_state) {
      if let Ok(elapsed) = self.start_time.elapsed() {
        elapsed > duration
      }
      else {
        // occurs if start_time is before now, can happen if system clock slips.
        // we'll eventually catch up so just ignore for now
        false
      }
    }
    else {
      false
    }
  }
}
