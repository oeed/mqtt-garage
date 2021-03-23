use core::panic;
use std::{collections::HashMap, convert::TryFrom};

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
  subscriptions: HashMap<String, fn(String) -> GarageResult<()>>,
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
      subscriptions: HashMap::new(),
    }
  }

  pub async fn subscribe(
    &mut self,
    topic: &str,
    qos: QoS,
    on_event: fn(String) -> GarageResult<()>,
  ) -> GarageResult<()> {
    if self.subscriptions.contains_key(topic) {
      panic!("attempted to subscribe to the same topic twice: {}", topic);
    }

    self.client.subscribe(topic, qos).await?;
    self.subscriptions.insert(String::from(topic), on_event);
    Ok(())
  }

  pub async fn publish(&self, topic: &str, qos: QoS, retain: bool, payload: &str) -> GarageResult<()> {
    self
      .client
      .publish(topic, qos, retain, payload)
      .await
      .map_err(|err| err.into())
  }

  pub async fn poll(mut self) -> GarageResult<()> {
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
        if let Some(subscription) = self.subscriptions.get(&message.topic) {
          if let Ok(payload) = String::from_utf8(message.payload.to_vec()) {
            // we only handle a payload if it is subscribed and can decode to a string, others are not related to us
            subscription(payload)?;
          }
        }
      }
    }
  }
}
