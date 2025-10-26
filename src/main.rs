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
pub mod health;
pub mod mqtt_client;
pub mod rgb;
pub mod wifi;


use health::{
  LAST_ASYNC_TICK_1S_S, LAST_ASYNC_TICK_S, LAST_DOOR_TICK_S, LAST_MQTT_RX_TICK_S, LAST_MQTT_TX_TICK_S, LAST_SECTION_ID,
  LAST_TIMER_THREAD_TICK_S, init_baseline, last_section_age, now_secs, section_name,
};

#[embassy_executor::task]
async fn timer_thread_heartbeat_task() {
  loop {
    embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
    LAST_TIMER_THREAD_TICK_S.store(now_secs(), Ordering::Relaxed);
  }
}


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
  esp_idf_svc::sys::link_patches();
  #[cfg(debug_assertions)]
  EspLogger::initialize_default();
  // release logger is configured in wifi.rs

  // Initialize global monotonic baseline for health timestamps
  init_baseline();

  // Global panic hook to log thread panics with last section info
  std::panic::set_hook(Box::new(|info| {
    let thread = std::thread::current();
    let tname = thread.name().unwrap_or("unnamed");
    let msg = info.to_string();
    let loc = info
      .location()
      .map(|l| (l.file(), l.line()))
      .unwrap_or(("<unknown>", 0));
    let sid = LAST_SECTION_ID.load(Ordering::Relaxed);
    log::error!(
      "THREAD PANIC: thread={} at {}:{}; msg={}; last_section={} age={}s; ages: async1s={}s async5m={}s mqtt_rx={}s mqtt_tx={}s door={}s",
      tname,
      loc.0,
      loc.1,
      msg,
      section_name(sid),
      last_section_age(),
      {
        let v = LAST_ASYNC_TICK_1S_S.load(Ordering::Relaxed);
        if v == 0 { u32::MAX } else { v }
      },
      {
        let v = LAST_ASYNC_TICK_S.load(Ordering::Relaxed);
        if v == 0 { u32::MAX } else { v }
      },
      {
        let v = LAST_MQTT_RX_TICK_S.load(Ordering::Relaxed);
        if v == 0 { u32::MAX } else { v }
      },
      {
        let v = LAST_MQTT_TX_TICK_S.load(Ordering::Relaxed);
        if v == 0 { u32::MAX } else { v }
      },
      {
        let v = LAST_DOOR_TICK_S.load(Ordering::Relaxed);
        if v == 0 { u32::MAX } else { v }
      }
    );
  }));

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

    // clear tickers
    LAST_ASYNC_TICK_S.store(0, Ordering::Relaxed);
    LAST_ASYNC_TICK_1S_S.store(0, Ordering::Relaxed);
    LAST_MQTT_RX_TICK_S.store(0, Ordering::Relaxed);
    LAST_MQTT_TX_TICK_S.store(0, Ordering::Relaxed);
    LAST_DOOR_TICK_S.store(0, Ordering::Relaxed);
    LAST_TIMER_THREAD_TICK_S.store(0, Ordering::Relaxed);

    log::info!("Reset reason: {:?}", ResetReason::get());
    log::info!("Wakeup reason: {:?}", WakeupReason::get());


    {
      // monitor async executor health and reboot if stalled
      std::thread::spawn(move || {
        let mut counter: u8 = 0;
        loop {
          std::thread::sleep(std::time::Duration::from_secs(20));
          counter = counter.wrapping_add(1);
          let sid = LAST_SECTION_ID.load(Ordering::Relaxed);
          if counter % 15 == 0 {
            log::info!("Thread heartbeat  last_section={} age={}s", section_name(sid), last_section_age());
          }


          let now_s = now_secs();
          let last_s = LAST_ASYNC_TICK_1S_S.load(Ordering::Relaxed);
          if last_s != 0 && now_s.saturating_sub(last_s) > 20 {
            let age = |a: &AtomicU32| {
              let v = a.load(Ordering::Relaxed);
              if v == 0 { u32::MAX } else { now_s.saturating_sub(v) }
            };
            log::error!(
              "Async stalled >3m; ages async5m={}s async1s={}s mqtt_rx={}s mqtt_tx={}s door={}s timer_thread={}s; last_section={} age={}s",
              age(&LAST_ASYNC_TICK_S),
              age(&LAST_ASYNC_TICK_1S_S),
              age(&LAST_MQTT_RX_TICK_S),
              age(&LAST_MQTT_TX_TICK_S),
              age(&LAST_DOOR_TICK_S),
              age(&LAST_TIMER_THREAD_TICK_S),
              section_name(sid),
              last_section_age(),
            );
            // wait to send log message before restarting
            std::thread::sleep(std::time::Duration::from_secs(1));
            esp_idf_svc::hal::reset::restart();
          }
        }
      });
    }

    let mqtt_channels = MqttChannels::new();

    // spawn a separate Embassy executor on its own std thread to test timer wakeups
    {
      std::thread::spawn(|| {
        let executor: &'static mut embassy_executor::Executor =
          Box::leak(Box::new(embassy_executor::Executor::new()));
        executor.run(|spawner| {
          let _ = spawner.spawn(timer_thread_heartbeat_task());
        });
      });
    }
    let MqttClient {
      receiver: mut mqtt_receiver,
      publisher: mut mqtt_publisher,
    } = MqttClient::new(&mqtt_channels, &mut rgb_led).await?;

    let result = select4(
      pin!(async move {
        LAST_MQTT_RX_TICK_S.store(now_secs(), Ordering::Relaxed);
        let r = mqtt_receiver.receive_messages().await;
        LAST_MQTT_RX_TICK_S.store(now_secs(), Ordering::Relaxed);
        r
      }),
      pin!(async move {
        LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);
        let r = mqtt_publisher.send_messages().await;
        LAST_MQTT_TX_TICK_S.store(now_secs(), Ordering::Relaxed);
        r
      }),
      pin!(async {
        Ok(
          Door::new(peripherals.pins.gpio14, &mqtt_channels, &mut rgb_led)
            .await?
            .listen()
            .await?,
        )
      }),
      pin!(async move {
        let mut count: u32 = 0;
        loop {
          embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
          let now_s = now_secs();
          LAST_ASYNC_TICK_1S_S.store(now_s, Ordering::Relaxed);
          count = count.wrapping_add(1);
          if count % 300 == 0 {
            LAST_ASYNC_TICK_S.store(now_s, Ordering::Relaxed);
            log::info!("Future heartbeat");
          }
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
