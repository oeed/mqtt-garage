use core::sync::atomic::Ordering;
use std::pin::pin;

use embassy_futures::select::{Either, select};
use embassy_sync::{
  blocking_mutex::raw::NoopRawMutex,
  channel::{Receiver, Sender},
};
use esp_idf_svc::mqtt::client::{EspAsyncMqttClient, QoS};

use crate::{
  config::CONFIG,
  error::GarageResult,
  health::{LAST_MQTT_TX_TICK_S, mark_section, now_secs, section},
  mqtt_client::{MqttChannels, MqttConnectionState},
};

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
  connection_state_channel: Receiver<'a, NoopRawMutex, MqttConnectionState, 4>,
}

impl<'a> MqttPublisher<'a> {
  pub fn new(client: EspAsyncMqttClient, channels: &'a MqttChannels) -> MqttPublisher<'a> {
    MqttPublisher {
      client,
      receive_channel: channels.publish_channel.receiver(),
      connection_state_channel: channels.connection_state_channel.receiver(),
    }
  }

  pub async fn publish(&mut self, publish: MqttPublish) -> GarageResult<()> {
    LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);
    let r = self
      .client
      .publish(publish.topic, publish.qos, publish.retain, publish.payload.as_bytes())
      .await;
    LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);
    r.map_err(|e| e.into()).map(|_| ())
  }

  pub async fn subscribe(&mut self) -> GarageResult<()> {
    log::info!("Subscribing to {}", CONFIG.door.sensor_topic);
    LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);
    mark_section(section::MQTT_TX_SUBSCRIBE);
    let _ = self
      .client
      .subscribe(&CONFIG.door.sensor_topic, QoS::AtLeastOnce)
      .await?;
    log::info!("Subscribing to {}", CONFIG.door.command_topic);
    LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);
    self
      .client
      .subscribe(&CONFIG.door.command_topic, QoS::AtLeastOnce)
      .await?;
    LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);

    Ok(())
  }

  pub async fn send_messages(&mut self) -> GarageResult<()> {
    // send announce and subscribe messages first; if broker isn't ready yet, retry
    self.on_connection_state_change(MqttConnectionState::Connected).await;

    loop {
      let result = select(
        pin!(async { self.receive_channel.receive().await }),
        pin!(async { self.connection_state_channel.receive().await }),
      )
      .await;

      match result {
        Either::First(publish) => {
          if let Err(err) = self.publish(publish).await {
            log::warn!("MQTT publish failed: {:?}; will retry announce/subscribe", err);
            // attempt to re-announce and re-subscribe before continuing
            let _ = self.announce().await;
            let _ = self.subscribe().await;
          }
        }
        Either::Second(connection_state) => {
          self.on_connection_state_change(connection_state).await;
        }
      }
    }
  }

  async fn on_connection_state_change(&mut self, connection_state: MqttConnectionState) {
    log::info!("MQTT connection state changed: {:?}", connection_state);
    match connection_state {
      MqttConnectionState::Connected => loop {
        match (self.announce().await, self.subscribe().await) {
          (Ok(()), Ok(())) => break,
          _ => {
            log::warn!("MQTT announce/subscribe failed; retrying shortly");
            embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
          }
        }
      },
      _ => (),
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
