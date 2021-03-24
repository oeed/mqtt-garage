use std::collections::HashMap;

use rumqttc::{AsyncClient, Event, EventLoop, Packet, QoS};
use tokio::sync::mpsc;

use super::{MqttPublish, PublishSender};
use crate::error::GarageResult;

pub type PublishReceiver = mpsc::UnboundedReceiver<MqttPublish>;

pub struct MqttReceiver {
  pub(super) client: AsyncClient,
  pub event_loop: EventLoop,
  /// The channel with which messages received from MQTT are fowarded on
  pub receive_channels: HashMap<String, PublishSender>,
}

impl MqttReceiver {
  pub async fn subscribe(&mut self, topic: String, qos: QoS) -> GarageResult<PublishReceiver> {
    if self.receive_channels.contains_key(&topic) {
      panic!("attempted to subscribe to the same channel twice");
    }

    self.client.subscribe(&topic, qos).await?;
    let (receive_tx, receive_rx) = mpsc::unbounded_channel();
    self.receive_channels.insert(topic, receive_tx);

    Ok(receive_rx)
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
}
