#![warn(rust_2018_idioms)]

use std::{fs, sync::Arc, time::Duration};

use mqtt_garage::{
  config::Config,
  door::{
    state_detector::{
      assumed::AssumedStateDetector, gpio::GpioStateDetector, zigbee2mqtt::Zigbee2MqttStateDetector, StateDetector,
      StateDetectorConfig,
    },
    Door, RemoteMutex,
  },
  error::GarageError,
  mqtt_client::MqttClient,
};
use simple_logger::SimpleLogger;
use tokio::{self, time::sleep};

#[tokio::main]
async fn main() {
  SimpleLogger::new()
    .with_module_level("rumqttc", log::LevelFilter::Warn)
    .init()
    .unwrap();

  loop {
    let err = run().await;
    log::error!("Error occurred, restarting in 5 seconds: {:?}", err);
    // wait some time for the broker to come back online
    sleep(Duration::from_secs(5)).await;
  }
}

/// Run the MQTT receiver and sender and react
/// Runs forever unless an error occurs
async fn run() -> GarageError {
  let config = fs::read_to_string("garage-config.toml").expect("unable to read garage-config.toml");
  let config: Config = toml::from_str(&config).expect("unable to parse garage-config.toml");

  let remote_mutex = Arc::new(RemoteMutex::new());

  let (send_channel, mut client) = MqttClient::with_config("mqtt-garage", config.mqtt_client);

  for (identifier, door_config) in config.doors {
    match door_config.state_detector {
      StateDetectorConfig::Assumed(state_detector) => {
        // TODO: some elegant way to do this without copy paste
        let door = Door::<AssumedStateDetector>::with_config(
          identifier.into(),
          door_config.command_topic,
          door_config.state_topic,
          door_config.stuck_topic,
          door_config.initial_target_state,
          door_config.remote,
          state_detector,
          send_channel.clone(),
          Arc::clone(&remote_mutex),
        )
        .await
        .expect("failed to initialised door");

        if let Err(err) = door.listen(&mut client.receiver).await {
          return err;
        }
      }

      StateDetectorConfig::Gpio(state_detector) => {
        // TODO: some elegant way to do this without copy paste
        let door = Door::<GpioStateDetector>::with_config(
          identifier.into(),
          door_config.command_topic,
          door_config.state_topic,
          door_config.stuck_topic,
          door_config.initial_target_state,
          door_config.remote,
          state_detector,
          send_channel.clone(),
          Arc::clone(&remote_mutex),
        )
        .await
        .expect("failed to initialised door");

        if let Err(err) = door.listen(&mut client.receiver).await {
          return err;
        }
      }

      StateDetectorConfig::Zigbee2Mqtt(state_detector) => {
        // TODO: some elegant way to do this without copy paste
        let door = Door::<Zigbee2MqttStateDetector>::with_config(
          identifier.into(),
          door_config.command_topic,
          door_config.state_topic,
          door_config.stuck_topic,
          door_config.initial_target_state,
          door_config.remote,
          state_detector,
          send_channel.clone(),
          Arc::clone(&remote_mutex),
        )
        .await
        .expect("failed to initialised door");

        if let Err(err) = door.listen(&mut client.receiver).await {
          return err;
        }
      }
    };
  }

  client.announce().await.expect("failed to announce client");

  let mut receiver = client.receiver;
  let receive = tokio::spawn(async move { receiver.receive_messages().await.unwrap() });

  let mut sender = client.sender;
  let send = tokio::spawn(async move { sender.send_messages().await.unwrap() });

  // the two tasks will only end if an error occurs (most likely MQTT broker disconnection)
  tokio::try_join!(receive, send).unwrap_err().into()
}
