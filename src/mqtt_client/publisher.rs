use embassy_sync::{
  blocking_mutex::raw::NoopRawMutex,
  channel::{Receiver, Sender},
};
use esp_idf_svc::mqtt::client::{EspAsyncMqttClient, QoS};

use crate::{config::CONFIG, error::GarageResult, mqtt_client::MqttChannels};

#[derive(Debug)]
pub struct MqttPublish {
  pub topic: &'static str,
  pub qos: QoS,
  pub retain: bool,
  pub payload: &'static str,
}


pub struct MqttTopicPublisher<'a> {
  pub(super) send_channel: Sender<'a, NoopRawMutex, MqttPublish, 4>,
}

impl<'a> MqttTopicPublisher<'a> {
  pub async fn publish(&self, publish: MqttPublish) {
    self.send_channel.send(publish).await
  }
}

pub struct MqttPublisher<'a> {
  client: EspAsyncMqttClient,
  receive_channel: Receiver<'a, NoopRawMutex, MqttPublish, 4>,
}

impl<'a> MqttPublisher<'a> {
  pub fn new(client: EspAsyncMqttClient, channels: &'a MqttChannels) -> MqttPublisher<'a> {
    MqttPublisher {
      client,
      receive_channel: channels.publish_channel.receiver(),
    }
  }

  pub async fn publish(&mut self, publish: MqttPublish) -> GarageResult<()> {
    self
      .client
      .publish(publish.topic, publish.qos, publish.retain, publish.payload.as_bytes())
      .await?;
    Ok(())
  }

  pub async fn subscribe(&mut self) -> GarageResult<()> {
    log::info!("Subscribing to {}", CONFIG.door.sensor_topic);
    self
      .client
      .subscribe(&CONFIG.door.sensor_topic, QoS::AtLeastOnce)
      .await?;
    log::info!("Subscribing to {}", CONFIG.door.command_topic);
    self
      .client
      .subscribe(&CONFIG.door.command_topic, QoS::AtLeastOnce)
      .await?;

    Ok(())
  }

  pub async fn send_messages(&mut self) -> GarageResult<()> {
    // send announce and subscribe messages first
    self.announce().await?;
    self.subscribe().await?;

    loop {
      let publish = self.receive_channel.receive().await;
      self.publish(publish).await?;
    }
  }

  /// Announce our availability
  pub async fn announce(&mut self) -> GarageResult<()> {
    // announce our availability
    self
      .publish(MqttPublish {
        topic: &CONFIG.mqtt.availability_topic,
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: &CONFIG.mqtt.online_availability,
      })
      .await
  }
}
