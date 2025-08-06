#![warn(rust_2018_idioms)]


use std::pin::pin;

use embassy_executor::Spawner;
use embassy_futures::select::{Either4, select4};
use esp_idf_svc::{
  eventloop::EspSystemEventLoop, hal::prelude::Peripherals, log::EspLogger, nvs::EspDefaultNvsPartition,
  timer::EspTimerService,
};

use crate::{
  door::Door,
  mqtt_client::{MqttChannels, MqttClient},
  rgb::RgbLed,
  wifi::Wifi,
};

pub mod config;
pub mod door;
pub mod error;
pub mod mqtt_client;
pub mod rgb;
pub mod wifi;


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
  esp_idf_svc::sys::link_patches();
  EspLogger::initialize_default();

  log::info!("Starting...");

  let sys_loop = EspSystemEventLoop::take().unwrap();
  let timer_service = EspTimerService::new().unwrap();
  let nvs = EspDefaultNvsPartition::take().unwrap();
  let peripherals = Peripherals::take().unwrap();

  // loop {
  let err = async {
    let mut rgb_led = RgbLed::new(peripherals.rmt.channel0, peripherals.pins.gpio48)?;
    let wifi = Wifi::connect(
      peripherals.modem,
      sys_loop.clone(),
      timer_service.clone(),
      nvs.clone(),
      &mut rgb_led,
    )
    .await?;
    let mqtt_channels = MqttChannels::new();
    let MqttClient {
      receiver: mut mqtt_receiver,
      publisher: mut mqtt_publisher,
    } = MqttClient::new(&mqtt_channels, &mut rgb_led).await?;


    let result = select4(
      pin!(async move { wifi.wait_for_disconnect().await }),
      pin!(async move { mqtt_receiver.receive_messages().await }),
      pin!(async move { mqtt_publisher.send_messages().await }),
      pin!(async {
        Ok(
          Door::new(peripherals.pins.gpio14, &mqtt_channels, &mut rgb_led)
            .await?
            .listen()
            .await?,
        )
      }),
    )
    .await;

    match result {
      Either4::First(Err(err)) | Either4::Second(Err(err)) | Either4::Third(Err(err)) | Either4::Fourth(Err(err)) => {
        Err::<(), _>(err)
      }
      _ => unreachable!(),
    }
  }
  .await
  .unwrap_err(); // never Ok

  // #[cfg(debug_assertions)]
  // log::error!("Fatal error: {:?}", err);

  // only restart if not in debug mode
  // #[cfg(not(debug_assertions))]
  {
    log::error!("Fatal error, restarting in 5 seconds: {:?}", err);
    // wait some time for the broker to come back online
    embassy_time::Timer::after(embassy_time::Duration::from_secs(5)).await;
    esp_idf_svc::hal::reset::restart()
  }
}
