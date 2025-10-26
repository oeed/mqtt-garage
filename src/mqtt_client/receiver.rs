use std::str::FromStr;

use embassy_sync::{
  blocking_mutex::raw::NoopRawMutex,
  channel::{Receiver, Sender},
};
use esp_idf_svc::mqtt::client::*;

use crate::{
  config::CONFIG,
  door::{SensorPayload, state::TargetState},
  error::GarageResult,
  health::{LAST_MQTT_RX_TICK_S, mark_section, now_secs, section},
  mqtt_client::{CHANNEL_SIZE, MqttChannels, MqttConnectionState},
};


pub type MqttTopicReceiver<'a, T> = Receiver<'a, NoopRawMutex, T, CHANNEL_SIZE>;

pub struct MqttReceiver<'a> {
  connection: EspAsyncMqttConnection,
  sensor_send_channel: Sender<'a, NoopRawMutex, SensorPayload, CHANNEL_SIZE>,
  command_send_channel: Sender<'a, NoopRawMutex, TargetState, CHANNEL_SIZE>,
  connection_state_send_channel: Sender<'a, NoopRawMutex, MqttConnectionState, CHANNEL_SIZE>,
}

impl<'a> MqttReceiver<'a> {
  pub fn new(connection: EspAsyncMqttConnection, channels: &'a MqttChannels) -> MqttReceiver<'a> {
    MqttReceiver {
      connection,
      sensor_send_channel: channels.sensor_channel.sender(),
      command_send_channel: channels.command_channel.sender(),
      connection_state_send_channel: channels.connection_state_channel.sender(),
    }
  }

  pub async fn receive_messages(&mut self) -> GarageResult<()> {
    // mark activity on entry
    LAST_MQTT_RX_TICK_S.store(now_secs(), core::sync::atomic::Ordering::Relaxed);
    loop {
      mark_section(section::MQTT_RX_NEXT);
      let event = self.connection.next().await?;
      LAST_MQTT_RX_TICK_S.store(now_secs(), core::sync::atomic::Ordering::Relaxed);
      match event.payload() {
        EventPayload::Received { topic, data, .. } => {
          if topic == Some(&CONFIG.door.sensor_topic)
            && let Ok((payload, _)) = serde_json_core::from_slice(data)
          {
            log::info!("Received sensor: {payload:?}");
            mark_section(section::MQTT_RX_SENSOR_SEND);
            self.sensor_send_channel.send(payload).await;
            LAST_MQTT_RX_TICK_S.store(now_secs(), core::sync::atomic::Ordering::Relaxed);
          }
          else if topic == Some(&CONFIG.door.command_topic)
            && let Ok(state) = str::from_utf8(data)
              .map_err(|_| ())
              .and_then(|str| TargetState::from_str(str))
          {
            log::info!("Received command: {state}");
            mark_section(section::MQTT_RX_COMMAND_SEND);
            self.command_send_channel.send(state).await;
            LAST_MQTT_RX_TICK_S.store(now_secs(), core::sync::atomic::Ordering::Relaxed);
          }
        }

        EventPayload::Connected(_) => {
          log::info!("MQTT connected");
          LAST_MQTT_RX_TICK_S.store(now_secs(), core::sync::atomic::Ordering::Relaxed);
          self
            .connection_state_send_channel
            .send(MqttConnectionState::Connected)
            .await;
        }

        EventPayload::Disconnected => {
          log::warn!("MQTT disconnected; waiting for reconnect");
          LAST_MQTT_RX_TICK_S.store(now_secs(), core::sync::atomic::Ordering::Relaxed);
          self
            .connection_state_send_channel
            .send(MqttConnectionState::Disconnected)
            .await;
        }

        _ => {}
      }
    }
  }
}
