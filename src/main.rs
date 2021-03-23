use std::{
  borrow::{Borrow, BorrowMut},
  fs,
};

use mqtt_garage::{config::Config, door::Door, mqtt_client::MqttClient};
use tokio;

#[tokio::main]
async fn main() {
  let config = fs::read_to_string("garage-config.toml").expect("unable to read garage-config.toml");
  let config: Config = toml::from_str(&config).expect("unable to parse garage-config.toml");

  let mut client = MqttClient::with_config(config.mqtt_client);
  let doors: Vec<_> = config
    .doors
    .into_iter()
    .map(|(identifier, door)| Door::with_config(identifier, door.state_detector, door.remote, door.mqtt));


  client
    .poll(async |topic, payload| {
      for door in doors {
        door.on_message(&topic, &payload).await?
      }
    })
    .await
    .unwrap();
}
