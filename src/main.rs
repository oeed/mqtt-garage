#![warn(rust_2018_idioms)]


use std::pin::pin;

use embassy_futures::select::{Either3, select3};
use esp_idf_svc::{eventloop::EspSystemEventLoop, log::EspLogger, nvs::EspDefaultNvsPartition, timer::EspTimerService};

// use tokio::{self, select, task::JoinSet, time::sleep};
use crate::{
  door::Door,
  mqtt_client::{MqttChannels, MqttClient},
  wifi::Wifi,
};

pub mod config;
pub mod door;
pub mod error;
pub mod mqtt_client;
pub mod wifi;


fn main() {
  esp_idf_svc::sys::link_patches();
  EspLogger::initialize_default();

  let sys_loop = EspSystemEventLoop::take().unwrap();
  let timer_service = EspTimerService::new().unwrap();
  let nvs = EspDefaultNvsPartition::take().unwrap();

  loop {
    let err = esp_idf_svc::hal::task::block_on(async {
      let _wifi = Wifi::connect(sys_loop.clone(), timer_service.clone(), nvs.clone()).await?;
      let mqtt_channels = MqttChannels::new();
      let MqttClient {
        receiver: mut mqtt_receiver,
        publisher: mut mqtt_publisher,
      } = MqttClient::new(&mqtt_channels).await?;
      let door = Door::new(&mqtt_channels).await?; // TODO: maybe timeout?

      mqtt_publisher.announce().await.expect("failed to announce client");

      let result = select3(
        pin!(async move { mqtt_receiver.receive_messages().await }),
        pin!(async move { mqtt_publisher.send_messages().await }),
        pin!(async move { door.listen().await }),
      )
      .await;

      match result {
        Either3::First(Err(err)) | Either3::Second(Err(err)) => Err::<(), _>(err),
        _ => unreachable!(),
      }
    })
    .unwrap_err(); // never Ok

    log::error!("Error occurred, restarting in 5 seconds: {:?}", err);
    // wait some time for the broker to come back online
    // sleep(Duration::from_secs(5)).await; // TODO: use a timer
  }
}
