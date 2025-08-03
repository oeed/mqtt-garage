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
  mqtt_client::{CHANNEL_SIZE, MqttChannels},
};


pub type MqttTopicReceiver<'a, T> = Receiver<'a, NoopRawMutex, T, CHANNEL_SIZE>;

pub struct MqttReceiver<'a> {
  connection: EspAsyncMqttConnection,
  sensor_send_channel: Sender<'a, NoopRawMutex, SensorPayload, CHANNEL_SIZE>,
  command_send_channel: Sender<'a, NoopRawMutex, TargetState, CHANNEL_SIZE>,
}

impl<'a> MqttReceiver<'a> {
  pub fn new(connection: EspAsyncMqttConnection, channels: &'a MqttChannels) -> MqttReceiver<'a> {
    MqttReceiver {
      connection,
      sensor_send_channel: channels.sensor_channel.sender(),
      command_send_channel: channels.command_channel.sender(),
    }
  }

  pub async fn receive_messages(&mut self) -> GarageResult<()> {
    loop {
      let event = self.connection.next().await?;
      if let EventPayload::Received { topic, data, .. } = event.payload() {
        log::info!("{topic:?}: {data:?}", data = String::from_utf8_lossy(data));
        if topic == Some(&CONFIG.door.sensor_topic)
          && let Ok((payload, _)) = serde_json_core::from_slice(data)
        {
          log::info!("Received sensor: {payload:?}");
          self.sensor_send_channel.send(payload).await;
        }
        else if topic == Some(&CONFIG.door.command_topic)
          && let Ok(state) = str::from_utf8(data)
            .map_err(|_| ())
            .and_then(|str| TargetState::from_str(str))
        {
          log::info!("Received command: {state}");
          self.command_send_channel.send(state).await;
        }
      }
    }
  }
}
