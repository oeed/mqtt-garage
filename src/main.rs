#![warn(rust_2018_idioms)]

use std::{fs, sync::Arc, time::Duration};

use simple_logger::SimpleLogger;
use tokio::{self, select, time::sleep};

use crate::{
  config::Config,
  door::{controller::remote::mutex::RemoteMutex, Door},
  error::GarageError,
  mqtt_client::MqttClient,
};

pub mod config;
pub mod door;
pub mod error;
#[cfg(not(feature = "arm"))]
mod mock_gpio;
pub mod mqtt_client;

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
async fn run() -> Result<(), GarageError> {
  let config = fs::read_to_string("garage-config.toml").expect("unable to read garage-config.toml");
  let config: Config = toml::from_str(&config).expect("unable to parse garage-config.toml");

  let remote_mutex = Arc::new(RemoteMutex::new());

  let (send_channel, mut client) = MqttClient::new("mqtt-garage", config.mqtt_client);

  let mut doors = Vec::with_capacity(config.doors.len());
  for (identifier, door_config) in config.doors {
    doors.push(
      Door::new(
        identifier.into(),
        door_config,
        send_channel.clone(),
        remote_mutex.clone(),
        &mut client.receiver,
      )
      .await?,
    );
  }

  client.announce().await.expect("failed to announce client");

  let mut receiver = client.receiver;
  let receive = tokio::spawn(async move { receiver.receive_messages().await.unwrap() });

  let mut sender = client.sender;
  let send = tokio::spawn(async move { sender.send_messages().await.unwrap() });

  // once the receiver and sender are running, we can start listening
  for door in doors {
    let identifier = door.identifier.clone();
    select! {
      _ = tokio::time::sleep(Duration::from_secs(10)) => return Err(GarageError::DoorInitialisationTimeout(identifier)),
      _ = door.listen() => {}
    };
  }

  // the two tasks will only end if an error occurs (most likely MQTT broker disconnection)
  Err(tokio::try_join!(receive, send).unwrap_err().into())
}
