use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};
use esp_idf_svc::mqtt::client::*;

pub use self::{
  publisher::{MqttPublish, MqttPublisher, MqttTopicPublisher},
  receiver::{MqttReceiver, MqttTopicReceiver},
};
use crate::{
  config::CONFIG,
  door::{SensorPayload, state::TargetState},
  error::GarageResult,
};

mod publisher;
mod receiver;

const CHANNEL_SIZE: usize = 4;

pub struct MqttChannels {
  /// The channel with which messages to send to MQTT are received on (from `MqttTopicPublisher`)
  publish_channel: Channel<NoopRawMutex, MqttPublish, CHANNEL_SIZE>, // TODO: need to assess whether the fixed limit will have issues
  sensor_channel: Channel<NoopRawMutex, SensorPayload, CHANNEL_SIZE>,
  command_channel: Channel<NoopRawMutex, TargetState, CHANNEL_SIZE>,
}

impl MqttChannels {
  pub fn new() -> MqttChannels {
    MqttChannels {
      publish_channel: Channel::new(),
      sensor_channel: Channel::new(),
      command_channel: Channel::new(),
    }
  }

  pub fn publisher(&self) -> MqttTopicPublisher<'_> {
    MqttTopicPublisher {
      send_channel: self.publish_channel.sender(),
    }
  }

  pub fn sensor_receiver(&self) -> MqttTopicReceiver<'_, SensorPayload> {
    self.sensor_channel.receiver()
  }

  pub fn command_receiver(&self) -> MqttTopicReceiver<'_, TargetState> {
    self.command_channel.receiver()
  }
}

pub struct MqttClient<'a> {
  pub receiver: MqttReceiver<'a>,
  pub publisher: MqttPublisher<'a>,
}

impl<'a> MqttClient<'a> {
  pub async fn new(channels: &'a MqttChannels) -> GarageResult<MqttClient<'a>> {
    log::info!("Creating MQTT client");
    let (mut client, connection) = EspAsyncMqttClient::new(
      &CONFIG.mqtt.url,
      &MqttClientConfiguration {
        client_id: Some(&CONFIG.mqtt.client_id),
        lwt: Some(LwtConfiguration {
          topic: &CONFIG.mqtt.availability_topic,
          payload: CONFIG.mqtt.offline_availability.as_ref().as_bytes(),
          qos: QoS::AtLeastOnce,
          retain: true,
        }),
        ..Default::default()
      },
    )?;

    Ok(MqttClient {
      receiver: MqttReceiver::new(&mut client, connection, channels).await?,
      publisher: MqttPublisher::new(client, channels),
    })
  }
}
