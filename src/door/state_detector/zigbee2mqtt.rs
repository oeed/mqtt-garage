use std::time::Duration;

use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};

use super::{DetectedState, StateDetector, Travel};
use crate::{
  door::{
    state::{State, TargetState},
    Identifier,
  },
  error::GarageResult,
  mqtt_client::{
    receiver::{MqttReceiver, PublishReceiver},
    MqttPublish,
  },
};

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Zigbee2MqttStateDetectorConfig {
  pub sensor_topic: String,

  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is expected to take to go to/from open/close.
  ///
  /// If it exceeds this it tries again.
  pub travel_time: Duration,
}

#[derive(Debug, Deserialize)]
struct ContactSensorPayload {
  /// `true` if closed
  contact: bool,
}


#[derive(Debug)]
pub struct Zigbee2MqttStateDetector {
  sensor_topic: String,
  travel_time: Duration,
  current_travel: Option<Travel>,
  /// The state of the last message received about the sensors state
  last_state: DetectedState,
}

impl StateDetector for Zigbee2MqttStateDetector {
  type Config = Zigbee2MqttStateDetectorConfig;

  fn with_config(_: Identifier, config: Self::Config) -> GarageResult<Self> {
    Ok(Zigbee2MqttStateDetector {
      sensor_topic: config.sensor_topic,
      travel_time: config.travel_time,
      last_state: DetectedState::Stuck, // stuck will be replaced with the value from MQTT (which should be set to retain)
      current_travel: None,
    })
  }

  async fn travel(&mut self, target_state: TargetState) -> DetectedState {
    if self.current_travel.is_some() {
      panic!("Zigbee2MqttStateDetectorConfig attempted to travel while it was already travelling");
    }
    self.current_travel = Some(Travel::new(target_state));
    tokio::time::sleep(self.travel_time).await;
    self.detect_state()
  }

  fn detect_state(&mut self) -> DetectedState {
    // check if this state indicates the door might be stuck
    if let Some(current_travel) = self.current_travel.take() {
      if current_travel.expired_invalid(self.last_state, self.travel_time) {
        return DetectedState::Stuck;
      }
    }

    self.last_state
  }

  fn should_check_periodically(&self) -> bool {
    true
  }

  async fn subscribe(&mut self, mqtt_receiver: &mut MqttReceiver) -> GarageResult<Option<PublishReceiver>> {
    log::debug!("Subscribing zigbee2mqtt sensor to topic '{}'", &self.sensor_topic);
    Ok(Some(
      mqtt_receiver
        .subscribe(self.sensor_topic.clone(), rumqttc::QoS::AtLeastOnce)
        .await?,
    ))
  }

  fn receive_message(&mut self, publish: MqttPublish) {
    if &self.sensor_topic == &publish.topic {
      serde_json::from_str::<ContactSensorPayload>(&publish.payload)
        .map(|payload| {
          log::debug!("Received sensor payload: {payload:?}");
          self.last_state = if payload.contact {
            DetectedState::Closed
          }
          else {
            DetectedState::Open
          };
        })
        .unwrap_or_else(|e| {
          log::error!("Failed to parse sensor payload: {}", e);
          self.last_state = DetectedState::Stuck;
        });
    }
  }

  fn manual_travel_state(&self, target_state: TargetState) -> State {
    match target_state {
      // the sensor indicates if it's closed, so once no longer closed we assume opening
      TargetState::Open => State::Opening,
      TargetState::Closed => State::Closed,
    }
  }
}
