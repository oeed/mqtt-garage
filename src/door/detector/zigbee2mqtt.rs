use serde::Deserialize;
use tokio::sync::mpsc::{self, UnboundedReceiver};

use super::{DetectedState, DoorDetector};
use crate::{
  door::identifier::Identifier,
  error::GarageResult,
  mqtt_client::{receiver::MqttReceiver, MqttPublish},
};

#[derive(Debug, Deserialize)]
pub struct Zigbee2MqttDoorDetectorConfig {
  pub sensor_topic: String,
}

#[derive(Debug)]
pub struct Zigbee2MqttDoorDetector {
  sensor_topic: String,
  mqtt_rx: UnboundedReceiver<MqttPublish>,
}

impl DetectedState {
  fn from_publish(sensor_topic: &str, publish: MqttPublish) -> Option<DetectedState> {
    if sensor_topic == &publish.topic {
      Some(
        serde_json::from_str::<ContactSensorPayload>(&publish.payload)
          .map(|payload| {
            log::debug!("Received sensor payload: {payload:?}");
            if payload.contact {
              DetectedState::Closed
            }
            else {
              DetectedState::Open
            }
          })
          .unwrap_or_else(|e| {
            log::error!("Failed to parse sensor payload: {}", e);
            DetectedState::Stuck
          }),
      )
    }
    else {
      None
    }
  }
}

impl DoorDetector for Zigbee2MqttDoorDetector {
  type Config = Zigbee2MqttDoorDetectorConfig;

  async fn new(_: Identifier, config: Self::Config, mqtt_receiver: &mut MqttReceiver) -> GarageResult<Self> {
    Ok(Zigbee2MqttDoorDetector {
      mqtt_rx: mqtt_receiver
        .subscribe(config.sensor_topic.clone(), rumqttc::QoS::AtLeastOnce)
        .await?,
      sensor_topic: config.sensor_topic,
    })
  }

  async fn listen(mut self) -> GarageResult<(DetectedState, mpsc::UnboundedReceiver<DetectedState>)> {
    log::debug!("Subscribing zigbee2mqtt sensor to topic '{}'", &self.sensor_topic);

    let (detector_tx, detector_rx) = mpsc::unbounded_channel();

    // read the initial state (this assumes it'll be retain and thus instantly available)
    let initial_state = loop {
      let publish = self.mqtt_rx.recv().await.expect("MQTT receiver ended");
      if let Some(initial_state) = DetectedState::from_publish(&self.sensor_topic, publish) {
        break initial_state;
      }
    };

    tokio::spawn(async move {
      loop {
        let publish = self.mqtt_rx.recv().await.expect("MQTT receiver ended");
        if let Some(detected_state) = DetectedState::from_publish(&self.sensor_topic, publish) {
          if detector_tx.send(detected_state).is_err() {
            // channel ended
            return;
          }
        }
      }
    });

    Ok((initial_state, detector_rx))
  }
}

#[derive(Debug, Deserialize)]
struct ContactSensorPayload {
  /// `true` if closed
  contact: bool,
}
