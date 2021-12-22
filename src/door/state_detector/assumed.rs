use std::{fs, str::FromStr, time::Duration};

use async_trait::async_trait;
use log::warn;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};

use super::{DetectedState, StateDetector, Travel};
use crate::{
  door::{state::TargetState, Identifier},
  error::GarageResult,
  mqtt_client::{
    receiver::{MqttReceiver, PublishReceiver},
    MqttPublish,
  },
};

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct AssumedStateDetectorConfig {
  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is assumed to take to go to/from open/close.
  pub travel_time: Duration,
  /// Top topic state overrides can be sent to to correct an incorrect state.
  pub override_topic: String,
}


#[derive(Debug)]
pub struct AssumedStateDetector {
  identifier: Identifier, // TODO: can we borrow this?
  travel_time: Duration,
  override_topic: String,
  current_travel: Option<Travel>,
  assumed_state: TargetState,
}

impl AssumedStateDetector {
  fn set_assumed_state(&mut self, assumed_state: TargetState) {
    self.assumed_state = assumed_state;
    self.current_travel = None;
    if let Err(err) = fs::write(format!("{}.state", &self.identifier.0), assumed_state.to_string()) {
      warn!("failed to write assumed state: {}", err);
    }
  }
}

#[async_trait]
impl StateDetector for AssumedStateDetector {
  type Config = AssumedStateDetectorConfig;

  fn with_config(identifier: Identifier, config: Self::Config) -> GarageResult<Self> {
    let assumed_state = fs::read_to_string(format!("{}.state", &identifier.0))
      .ok()
      .and_then(|value| TargetState::from_str(&value).ok())
      .unwrap_or(TargetState::Closed);

    Ok(AssumedStateDetector {
      identifier,
      travel_time: config.travel_time,
      override_topic: config.override_topic,
      current_travel: None,
      assumed_state,
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
      if current_travel.expired_invalid(self.assumed_state.into(), self.travel_time) {
        let target_state = current_travel.target_state;
        // door was moving and should've finished by now, we assume it's finished. move to the target state
        self.set_assumed_state(target_state);
        // TODO: write to file
        self.assumed_state.into()
      }
      else {
        // door is moving, use the previous state (which is the inverse of the target state)
        self.assumed_state.into()
      }
    }
    else {
      // the door isn't moving, so we use the previously assumed state
      self.assumed_state.into()
    }
  }

  fn should_check(&self) -> bool {
    true
  }

  async fn subscribe(&mut self, mqtt_receiver: &mut MqttReceiver) -> GarageResult<Option<PublishReceiver>> {
    Ok(Some(
      mqtt_receiver
        .subscribe(self.override_topic.clone(), rumqttc::QoS::AtLeastOnce)
        .await?,
    ))
  }

  fn receive_message(&mut self, publish: MqttPublish) {
    if &self.override_topic == &publish.topic {
      if let Ok(override_state) = TargetState::from_str(&publish.payload) {
        log::info!("{:?} overriding state to {:?}", &self.identifier, override_state);
        self.set_assumed_state(override_state);
      }
    }
  }
}
