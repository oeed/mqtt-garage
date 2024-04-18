use std::{fmt::Debug, future::Future};

use serde::Deserialize;
use tokio::sync::mpsc;

use self::zigbee2mqtt::{Zigbee2MqttDoorDetector, Zigbee2MqttDoorDetectorConfig};
use super::{identifier::Identifier, state::DetectedState};
use crate::{error::GarageResult, mqtt_client::receiver::MqttReceiver};

// pub mod assumed;
// pub mod gpio;
pub mod zigbee2mqtt;

pub trait DoorDetector: Debug {
  type Config;

  fn new(
    identifier: Identifier,
    config: Self::Config,
    mqtt_receiver: &mut MqttReceiver,
  ) -> impl Future<Output = GarageResult<Self>> + Send
  where
    Self: Sized;

  /// Listen to state changes, sending any changes along the returned channel.
  ///
  /// Must also return an initial state.
  fn listen(self)
    -> impl Future<Output = GarageResult<(DetectedState, mpsc::UnboundedReceiver<DetectedState>)>> + Send;
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DoorDetectorConfig {
  // Gpio(GpioDoorDetectorConfig),
  // Assumed(AssumedDoorDetectorConfig),
  Zigbee2Mqtt(Zigbee2MqttDoorDetectorConfig),
}

#[derive(Debug)]
pub enum AnyDoorDetector {
  // Gpio(GpioDoorDetector),
  // Assumed(AssumedDoorDetector),
  Zigbee2Mqtt(Zigbee2MqttDoorDetector),
}

impl DoorDetector for AnyDoorDetector {
  type Config = DoorDetectorConfig;

  async fn new(identifier: Identifier, config: Self::Config, mqtt_receiver: &mut MqttReceiver) -> GarageResult<Self> {
    match config {
      // DoorDetectorConfig::Gpio(config) => Ok(AnyDoorDetector::Gpio(GpioDoorDetector::new(identifier, config)?)),
      // DoorDetectorConfig::Assumed(config) => {
      //   Ok(AnyDoorDetector::Assumed(AssumedDoorDetector::new(identifier, config)?))
      // }
      DoorDetectorConfig::Zigbee2Mqtt(config) => Ok(AnyDoorDetector::Zigbee2Mqtt(
        Zigbee2MqttDoorDetector::new(identifier, config, mqtt_receiver).await?,
      )),
    }
  }

  fn listen(
    self,
  ) -> impl Future<Output = GarageResult<(DetectedState, mpsc::UnboundedReceiver<DetectedState>)>> + Send {
    match self {
      // AnyDoorDetector::Gpio(detector) => detector.listen(),
      // AnyDoorDetector::Assumed(detector) => detector.listen(),
      AnyDoorDetector::Zigbee2Mqtt(detector) => detector.listen(),
    }
  }
}
