#![warn(rust_2018_idioms)]


use std::pin::pin;

use embassy_executor::Spawner;
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


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
  esp_idf_svc::sys::link_patches();
  EspLogger::initialize_default();

  log::info!("Starting...");

  let sys_loop = EspSystemEventLoop::take().unwrap();
  let timer_service = EspTimerService::new().unwrap();
  let nvs = EspDefaultNvsPartition::take().unwrap();

  // loop {
  let err = async {
    let _wifi = Wifi::connect(sys_loop.clone(), timer_service.clone(), nvs.clone()).await?;
    let mqtt_channels = MqttChannels::new();
    let MqttClient {
      receiver: mut mqtt_receiver,
      publisher: mut mqtt_publisher,
    } = MqttClient::new(&mqtt_channels).await?;


    let result = select3(
      pin!(async move { mqtt_receiver.receive_messages().await }),
      pin!(async move { mqtt_publisher.send_messages().await }),
      pin!(async { Ok(Door::new(&mqtt_channels).await?.listen().await) }),
    )
    .await;

    match result {
      Either3::First(Err(err)) | Either3::Second(Err(err)) | Either3::Third(Err(err)) => Err::<(), _>(err),
      _ => unreachable!(),
    }
  }
  .await
  .unwrap_err(); // never Ok

  #[cfg(debug_assertions)]
  log::error!("Fatal error: {:?}", err);

  // only restart if not in debug mode
  #[cfg(not(debug_assertions))]
  {
    log::error!("Fatal error, restarting in 5 seconds: {:?}", err);
    // wait some time for the broker to come back online
    embassy_time::Timer::after(embassy_time::Duration::from_secs(5)).await;
    esp_idf_svc::hal::reset::restart()
  }
}
