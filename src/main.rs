use std::{borrow::BorrowMut, fs};

use mqtt_garage::{config::Config, mqtt_client::MqttClient};
use tokio;

#[tokio::main]
async fn main() {
  let config = fs::read_to_string("garage-config.toml").expect("unable to read garage-config.toml");
  let config: Config = toml::from_str(&config).expect("unable to parse garage-config.toml");

  let mut client = MqttClient::with_config(config.mqtt_client);

  client
    .publish("oliver-test/write", rumqttc::QoS::AtLeastOnce, false, "test")
    .await
    .unwrap();

  client
    .borrow_mut()
    .subscribe("oliver-test/read", rumqttc::QoS::AtLeastOnce, |body| {
      println!("got body: {}", body);
      Ok(())
    })
    .await
    .unwrap();

  client.poll().await.unwrap();
}
