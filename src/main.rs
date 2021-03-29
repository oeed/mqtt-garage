use std::{
  fs,
  sync::{Arc, Mutex, RwLock},
};

use mqtt_garage::{
  config::Config,
  door::{
    state_detector::{assumed::AssumedStateDetector, sensor::SensorStateDetector, StateDetector, StateDetectorConfig},
    Door, RemoteMutex,
  },
  mqtt_client::MqttClient,
};
use simple_logger::SimpleLogger;
use tokio;

#[tokio::main]
async fn main() {
  SimpleLogger::new()
    .with_module_level("rumqttc", log::LevelFilter::Warn)
    .init()
    .unwrap();

  let config = fs::read_to_string("garage-config.toml").expect("unable to read garage-config.toml");
  let config: Config = toml::from_str(&config).expect("unable to parse garage-config.toml");

  let remote_mutex = Arc::new(RemoteMutex::new());

  let (send_channel, mut client) = MqttClient::with_config(config.mqtt_client);

  for (identifier, door_config) in config.doors {
    match door_config.state_detector {
      StateDetectorConfig::Assumed(state_detector) => {
        // TODO: some elegant way to do this without copy paste
        let mut door = Door::<AssumedStateDetector>::with_config(
          identifier.into(),
          door_config.command_topic,
          door_config.state_topic,
          door_config.initial_target_state,
          door_config.remote,
          state_detector,
          send_channel.clone(),
          Arc::clone(&remote_mutex),
        )
        .await
        .expect("failed to initialised door");

        let receive_channel = door.subscribe(&mut client.receiver).await.unwrap();

        tokio::spawn(async move { door.listen(receive_channel).await });
      }

      StateDetectorConfig::Sensor(state_detector) => {
        // TODO: some elegant way to do this without copy paste
        let mut door = Door::<SensorStateDetector>::with_config(
          identifier.into(),
          door_config.command_topic,
          door_config.state_topic,
          door_config.initial_target_state,
          door_config.remote,
          state_detector,
          send_channel.clone(),
          Arc::clone(&remote_mutex),
        )
        .await
        .expect("failed to initialised door");

        let receive_channel = door.subscribe(&mut client.receiver).await.unwrap();

        tokio::spawn(async move { door.listen(receive_channel).await });
      }
    };
  }

  client.announce().await.expect("failed to announce client");

  let mut receiver = client.receiver;
  tokio::spawn(async move { receiver.receive_messages().await.unwrap() });

  let mut sender = client.sender;
  sender.send_messages().await.unwrap();
}
