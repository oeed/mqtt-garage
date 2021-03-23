use core::panic;
use std::{
  borrow::Borrow,
  collections::{HashMap, HashSet},
  convert::TryFrom,
  fmt::{self, Debug},
  future::Future,
  pin::Pin,
};

use rumqttc::{AsyncClient, Event, EventLoop, Incoming, LastWill, MqttOptions, Packet, QoS};
use serde::Deserialize;

use crate::error::GarageResult;

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

pub struct MqttClient {
  client: AsyncClient,
  event_loop: EventLoop,
  availability_topic: String,
  online_availability: String,
  /// A list of all topics that have been subscribed to
  subscribed_topics: HashSet<String>,
}

impl Debug for MqttClient {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "MqttClient")
  }
}

impl MqttClient {
  pub fn with_config(config: MqttClientConfig) -> Self {
    let mut mqttoptions = MqttOptions::new("mqtt-garage", config.broker_domain, config.broker_port);
    mqttoptions.set_last_will(LastWill::new(
      &config.availability_topic,
      config.offline_availability,
      QoS::AtLeastOnce,
      true,
    ));
    mqttoptions.set_keep_alive(5);

    let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

    MqttClient {
      client,
      event_loop,
      availability_topic: config.availability_topic,
      online_availability: config.online_availability,
      subscribed_topics: HashSet::new(),
    }
  }

  pub async fn subscribe(&mut self, topic: String, qos: QoS) -> GarageResult<()> {
    self.client.subscribe(&topic, qos).await?;
    self.subscribed_topics.insert(topic);
    Ok(())
  }

  pub async fn publish(&self, topic: &str, qos: QoS, retain: bool, payload: &str) -> GarageResult<()> {
    self
      .client
      .publish(topic, qos, retain, payload)
      .await
      .map_err(|err| err.into())
  }

  pub async fn pol(mut self, on_message: fn(String, String) -> GarageResult<()>) -> GarageResult<()> {
    // announce our availability
    self
      .publish(
        &self.availability_topic,
        QoS::AtLeastOnce,
        true,
        &self.online_availability,
      )
      .await
      .unwrap();

    loop {
      let notification = self.event_loop.poll().await?;
      println!("Received = {:?}", notification);
      if let Event::Incoming(Packet::Publish(message)) = notification {
        if self.subscribed_topics.contains(&message.topic) {
          if let Ok(payload) = String::from_utf8(message.payload.to_vec()) {
            on_message(message.topic, payload)?
          }
        }
      }
    }
  }
}
