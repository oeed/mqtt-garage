use rumqttc::{AsyncClient, QoS};
use tokio::sync::mpsc;

use super::{receiver::PublishReceiver, MqttPublish};
use crate::error::{GarageError, GarageResult};

pub type PublishSender = mpsc::UnboundedSender<MqttPublish>;

pub struct MqttSender {
  client: AsyncClient,
  /// The channel with which messages to send to MQTT are received on
  send_channel: PublishReceiver,
}

impl MqttSender {
  pub async fn publish(&self, topic: &str, qos: QoS, retain: bool, payload: &str) -> GarageResult<()> {
    self
      .client
      .publish(topic, qos, retain, payload)
      .await
      .map_err(|err| err.into())
  }

  pub async fn send_messages(&mut self) -> GarageResult<()> {
    loop {
      if let Some(publish) = self.send_channel.recv().await {
        self
          .client
          .publish(publish.topic, publish.qos, publish.retain, publish.payload)
          .await
          .map_err(|err| GarageError::from(err))?;
      }
      else {
        return Ok(());
      }
    }
  }
}
