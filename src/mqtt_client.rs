use std::{
  collections::HashMap,
  fmt::{self, Debug},
  time::Duration,
};

use rumqttc::{AsyncClient, LastWill, MqttOptions, QoS};
use serde::Deserialize;
use tokio::sync::mpsc;

use self::{
  receiver::MqttReceiver,
  sender::{MqttSender, PublishSender},
};
use crate::error::GarageResult;

pub mod receiver;
pub mod sender;

#[derive(Debug, Deserialize)]
pub struct MqttClientConfig {
  /// The domain name of the broker
  pub broker_domain: String,
  /// The port of the broker, 1883 by default
  pub broker_port: u16,
  /// The name of the MQTT topic availability states are sent on
  pub availability_topic: String,
  /// The payload of the state indicating the door is online
  pub online_availability: String,
  /// The payload of the state indicating the door is offline
  pub offline_availability: String,
}

#[derive(Debug)]
pub struct MqttPublish {
  pub topic: String,
  pub qos: QoS,
  pub retain: bool,
  pub payload: String,
}

pub struct MqttClient {
  availability_topic: String,
  online_availability: String,
  pub sender: MqttSender,
  pub receiver: MqttReceiver,
  pub client: AsyncClient,
}

impl Debug for MqttClient {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "MqttClient")
  }
}

impl MqttClient {
  pub fn new(id: &'static str, config: MqttClientConfig) -> (PublishSender, Self) {
    let mut mqttoptions = MqttOptions::new(id, config.broker_domain, config.broker_port);
    mqttoptions.set_last_will(LastWill::new(
      &config.availability_topic,
      config.offline_availability,
      QoS::AtLeastOnce,
      true,
    ));
    mqttoptions.set_keep_alive(Duration::from_secs(30));

    let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

    let (send_tx, send_rx) = mpsc::unbounded_channel();

    (
      send_tx,
      MqttClient {
        availability_topic: config.availability_topic,
        online_availability: config.online_availability,
        receiver: MqttReceiver {
          client: client.clone(),
          event_loop,
          receive_channels: HashMap::new(),
        },
        sender: MqttSender {
          client: client.clone(),
          send_channel: send_rx,
        },
        client,
      },
    )
  }

  /// Announce our availability
  pub async fn announce(&mut self) -> GarageResult<()> {
    // announce our availability
    self
      .sender
      .publish(
        &self.availability_topic,
        QoS::AtLeastOnce,
        false,
        &self.online_availability,
      )
      .await
  }
}
