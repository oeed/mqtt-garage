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
  health::{mark_mqtt_connected, mark_mqtt_disconnected},
  mqtt_client::{CHANNEL_SIZE, MqttChannels, MqttConnectionState},
};


pub type MqttTopicReceiver<'a, T> = Receiver<'a, NoopRawMutex, T, CHANNEL_SIZE>;

pub struct MqttReceiver<'a> {
  connection: EspAsyncMqttConnection,
  open_sensor_send_channel: Sender<'a, NoopRawMutex, SensorPayload, CHANNEL_SIZE>,
  closed_sensor_send_channel: Sender<'a, NoopRawMutex, SensorPayload, CHANNEL_SIZE>,
  command_send_channel: Sender<'a, NoopRawMutex, TargetState, CHANNEL_SIZE>,
  safe_to_close_send_channel: Sender<'a, NoopRawMutex, bool, CHANNEL_SIZE>,
  connection_state_send_channel: Sender<'a, NoopRawMutex, MqttConnectionState, CHANNEL_SIZE>,
}

impl<'a> MqttReceiver<'a> {
  pub fn new(connection: EspAsyncMqttConnection, channels: &'a MqttChannels) -> MqttReceiver<'a> {
    MqttReceiver {
      connection,
      open_sensor_send_channel: channels.open_sensor_channel.sender(),
      closed_sensor_send_channel: channels.closed_sensor_channel.sender(),
      command_send_channel: channels.command_channel.sender(),
      safe_to_close_send_channel: channels.safe_to_close_channel.sender(),
      connection_state_send_channel: channels.connection_state_channel.sender(),
    }
  }

  pub async fn receive_messages(&mut self) -> GarageResult<()> {
    loop {
      let event = self.connection.next().await?;
      match event.payload() {
        EventPayload::Received { topic, data, .. } => {
          if topic == Some(&CONFIG.door.open_sensor_topic)
            && let Ok((payload, _)) = serde_json_core::from_slice(data)
          {
            log::info!("Received open sensor: {payload:?}");
            self.open_sensor_send_channel.send(payload).await;
          }
          else if topic == Some(&CONFIG.door.closed_sensor_topic)
            && let Ok((payload, _)) = serde_json_core::from_slice(data)
          {
            log::info!("Received closed sensor: {payload:?}");
            self.closed_sensor_send_channel.send(payload).await;
          }
          else if topic == Some(&CONFIG.door.command_topic)
            && let Ok(state) = str::from_utf8(data)
              .map_err(|_| ())
              .and_then(|str| TargetState::from_str(str))
          {
            log::info!("Received command: {state}");
            self.command_send_channel.send(state).await;
          }
          else if topic == Some(&CONFIG.door.safe_to_close_topic)
            && let Ok(value) = str::from_utf8(data)
          {
            let safe = value == "true";
            log::info!("Received safe_to_close: {safe}");
            self.safe_to_close_send_channel.send(safe).await;
          }
        }

        EventPayload::Connected(_) => {
          log::info!("MQTT connected");
          // End-to-end connectivity is healthy again: disarm the connectivity watchdog.
          mark_mqtt_connected();
          self
            .connection_state_send_channel
            .send(MqttConnectionState::Connected)
            .await;
        }

        EventPayload::Disconnected => {
          log::warn!("MQTT disconnected; waiting for reconnect");
          // Arm the connectivity watchdog: if this outage persists the device reboots, even
          // though the Wi-Fi supervisor will normally restore the link well before then.
          mark_mqtt_disconnected();
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
