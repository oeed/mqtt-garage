use std::{fmt::Debug, future::Future};

use serde::Deserialize;

use super::{DetectedState, DoorDetector};
use crate::{
  door::identifier::Identifier,
  error::{GarageError, GarageResult},
  mqtt_client::{MqttPublish, receiver::MqttTopicReceiver},
};

#[derive(Debug, Deserialize)]
pub struct Zigbee2MqttDoorDetectorConfig {
  pub sensor_topic: String,
}

/// Senses the state of the door from Zigbee2MQTT
#[derive(Debug)]
pub struct DoorSensor {
  sensor_receiver: MqttTopicReceiver<'a, SensorState>,
}

impl DoorDetector for Zigbee2MqttDoorDetector {
  type Config = Zigbee2MqttDoorDetectorConfig;

  async fn new(_: Identifier, config: Self::Config, mqtt_receiver: &mut MqttTopicReceiver) -> GarageResult<Self> {
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
      let publish = self.mqtt_rx.recv().await.ok_or(GarageError::MqttClosed)?;
      if let Some(initial_state) = DetectedState::from_publish(&self.sensor_topic, publish) {
        break initial_state;
      }
    };

    tokio::spawn(async move {
      loop {
        if let Some(publish) = self.mqtt_rx.recv().await {
          if let Some(detected_state) = DetectedState::from_publish(&self.sensor_topic, publish) {
            if detector_tx.send(detected_state).is_err() {
              // channel ended
              break;
            }
          }
        }
        else {
          // channel ended
          break;
        }
      }
    });

    Ok((initial_state, detector_rx))
  }
}
