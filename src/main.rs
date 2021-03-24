use std::{
  borrow::{Borrow, BorrowMut},
  fs,
};

use mqtt_garage::{
  config::Config,
  door::{Door, RemoteMutex},
  mqtt_client::MqttClient,
};
use tokio;

#[tokio::main]
async fn main() {
  let config = fs::read_to_string("garage-config.toml").expect("unable to read garage-config.toml");
  let config: Config = toml::from_str(&config).expect("unable to parse garage-config.toml");

  let remote_mutex = RemoteMutex::new();

  let client = &mut MqttClient::with_config(config.mqtt_client);
  let doors: Vec<_> = try_join_all(
    config
      .doors
      .into_iter()
      .map(|(identifier, door)| ConcreteDoor::with_config(identifier, door, client, &remote_mutex)),
  )
  .await
  .expect("unable to initialise doors");


  client
    .poll(|topic, payload| {
      // process messages concurrently
      // we assume no door will use the same topic and thus only future will take a significant time
      tokio::spawn(async move {
        for door in doors {
          door
            .on_message(&topic, &payload)
            .await
            .expect("door message handling resulted in error")
        }
      });
    })
    .await;
}
