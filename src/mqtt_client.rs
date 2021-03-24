use core::panic;
use std::{
  borrow::Borrow,
  collections::{HashMap, HashSet},
  convert::TryFrom,
  fmt::{self, Debug},
  future::Future,
  pin::Pin,
};

use futures::channel::mpsc::UnboundedReceiver;
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, LastWill, MqttOptions, Packet, Publish, QoS};
use serde::Deserialize;
use tokio::{sync::mpsc, task};

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

pub type PublishSender = mpsc::UnboundedSender<MqttPublish>;
pub type PublishReceiver = mpsc::UnboundedReceiver<MqttPublish>;

#[derive(Debug)]
pub struct MqttPublish {
  pub topic: String,
  pub qos: QoS,
  pub retain: bool,
  pub payload: String,
}

pub struct MqttClient {
  client: AsyncClient,
  event_loop: EventLoop,
  availability_topic: String,
  online_availability: String,
  /// The channel with which messages to send to MQTT are received on
  send_channel: PublishReceiver,
  /// The channel with which messages received from MQTT are fowarded on
  receive_channels: HashMap<String, PublishSender>,
}

impl Debug for MqttClient {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "MqttClient")
  }
}

impl MqttClient {
  pub fn with_config(config: MqttClientConfig) -> (PublishSender, Self) {
    let mut mqttoptions = MqttOptions::new("mqtt-garage", config.broker_domain, config.broker_port);
    mqttoptions.set_last_will(LastWill::new(
      &config.availability_topic,
      config.offline_availability,
      QoS::AtLeastOnce,
      true,
    ));
    mqttoptions.set_keep_alive(5);

    let (client, event_loop) = AsyncClient::new(mqttoptions, 10);

    let (send_tx, send_rx) = mpsc::unbounded_channel();

    (
      send_tx,
      MqttClient {
        client,
        event_loop,
        availability_topic: config.availability_topic,
        online_availability: config.online_availability,
        send_channel: send_rx,
        receive_channels: HashMap::new(),
      },
    )
  }

  pub async fn subscribe(&mut self, topic: String, qos: QoS) -> GarageResult<PublishReceiver> {
    if self.receive_channels.contains_key(&topic) {
      panic!("attempted to subscribe to the same channel twice");
    }

    self.client.subscribe(&topic, qos).await?;
    let (receive_tx, receive_rx) = mpsc::unbounded_channel();
    self.receive_channels.insert(topic, receive_tx);

    Ok(receive_rx)
  }

  pub async fn publish(&self, topic: &str, qos: QoS, retain: bool, payload: &str) -> GarageResult<()> {
    self
      .client
      .publish(topic, qos, retain, payload)
      .await
      .map_err(|err| err.into())
  }

  /// Announce our availability
  pub async fn announce(&mut self) -> GarageResult<()> {
    // announce our availability
    self
      .publish(
        &self.availability_topic,
        QoS::AtLeastOnce,
        true,
        &self.online_availability,
      )
      .await
  }

  pub async fn receive_messages(&mut self) -> GarageResult<()> {
    loop {
      let notification = self.event_loop.poll().await?;
      println!("Received = {:?}", notification);
      if let Event::Incoming(Packet::Publish(message)) = notification {
        if let Some(channel) = self.receive_channels.get(&message.topic) {
          if let Ok(payload) = String::from_utf8(message.payload.to_vec()) {
            channel.send(MqttPublish {
              topic: message.topic,
              qos: message.qos,
              retain: message.retain,
              payload,
            });
          }
        }
      }
    }
  }

  pub async fn send_messages(&mut self) -> GarageResult<()> {
    loop {
      if let Some(publish) = self.send_channel.recv().await {
        self
          .client
          .publish(publish.topic, publish.qos, publish.retain, publish.payload)
          .await
          .map_err(|err| err.into())?;
      }
      else {
        return Ok(());
      }
    }
  }
}
