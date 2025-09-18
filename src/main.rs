#![warn(rust_2018_idioms)]

use core::sync::atomic::{AtomicU32, Ordering};
use std::pin::pin;

use embassy_executor::Spawner;
use embassy_futures::select::{Either4, select4};
#[cfg(debug_assertions)]
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::{
  eventloop::EspSystemEventLoop,
  hal::{
    prelude::Peripherals,
    reset::{ResetReason, WakeupReason},
  },
  nvs::EspDefaultNvsPartition,
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


static LAST_ASYNC_TICK_S: AtomicU32 = AtomicU32::new(0);


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
  esp_idf_svc::sys::link_patches();
  #[cfg(debug_assertions)]
  EspLogger::initialize_default();
  // release logger is configured in wifi.rs

  log::info!("Starting...");

  let sys_loop = EspSystemEventLoop::take().unwrap();
  let timer_service = EspTimerService::new().unwrap();
  let nvs = EspDefaultNvsPartition::take().unwrap();
  let peripherals = Peripherals::take().unwrap();

  // loop {
  let err = async {
    let mut rgb_led = RgbLed::new(peripherals.rmt.channel0, peripherals.pins.gpio48)?;
    let _wifi = Wifi::connect(
      peripherals.modem,
      sys_loop.clone(),
      timer_service.clone(),
      nvs.clone(),
      &mut rgb_led,
    )
    .await?;


    // initialize async heartbeat timestamp baseline (no unsafe)
    let start = std::time::Instant::now();
    LAST_ASYNC_TICK_S.store(0, Ordering::Relaxed);

    // If a previous run crashed and a core dump was saved to flash,
    // retrieve its flash address/size, log summary, then erase it.
    unsafe {
      use esp_idf_svc::sys;
      if sys::esp_core_dump_image_check() == sys::ESP_OK as i32 {
        let mut flash_addr: usize = 0;
        let mut flash_size: usize = 0;
        let rc = sys::esp_core_dump_image_get(&mut flash_addr as *mut usize, &mut flash_size as *mut usize);
        if rc == sys::ESP_OK as i32 && flash_size > 0 {
          log::warn!(
            "Core dump present at flash 0x{:X}, size {} bytes",
            flash_addr,
            flash_size
          );
        }
        else {
          log::warn!("Core dump present but could not get address/size (rc={})", rc);
        }
        let _ = sys::esp_core_dump_image_erase();
      }
    }

    log::info!("Reset reason: {:?}", ResetReason::get());
    log::info!("Wakeup reason: {:?}", WakeupReason::get());


    {
      // monitor async executor health and reboot if stalled
      std::thread::spawn(move || {
        let mut counter: u8 = 0;
        loop {
          std::thread::sleep(std::time::Duration::from_secs(60));
          counter = counter.wrapping_add(1);
          if counter % 5 == 0 {
            log::info!("Thread heartbeat");
          }

          let now_s = start.elapsed().as_secs() as u32;
          let last_s = LAST_ASYNC_TICK_S.load(Ordering::Relaxed);
          // if async heartbeat has not updated in 3 minutes, restart
          if last_s != 0 && now_s.saturating_sub(last_s) > 6 * 60 {
            log::error!(
              "Async executor stalled >3m (last tick {} s ago); restarting",
              now_s.saturating_sub(last_s)
            );
            esp_idf_svc::hal::reset::restart();
          }
        }
      });
    }

    let mqtt_channels = MqttChannels::new();
    let MqttClient {
      receiver: mut mqtt_receiver,
      publisher: mut mqtt_publisher,
    } = MqttClient::new(&mqtt_channels, &mut rgb_led).await?;

    let result = select4(
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
      pin!(async move {
        loop {
          embassy_time::Timer::after(embassy_time::Duration::from_secs(5 * 60)).await;
          let now_s = start.elapsed().as_secs() as u32;
          LAST_ASYNC_TICK_S.store(now_s, Ordering::Relaxed);
          log::info!("Future heartbeat");
        }
        #[allow(unreachable_code)]
        Ok(())
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
