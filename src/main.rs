#![warn(rust_2018_idioms)]

use core::sync::atomic::{AtomicU32, Ordering};
use std::pin::pin;

use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either4, select, select4};
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
  error::GarageError,
  mqtt_client::{MqttChannels, MqttClient},
  rgb::RgbLed,
  wifi::Wifi,
};

pub mod config;
pub mod diagnostics;
pub mod door;
pub mod error;
pub mod health;
pub mod log_storage;
pub mod mqtt_client;
pub mod rgb;
pub mod time_sync;
pub mod wifi;


use health::{LAST_ASYNC_TICK_1S_S, LAST_TIMER_THREAD_TICK_S, MQTT_DISCONNECTED_SINCE_S, init_baseline, now_secs};

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

  // Initialize global monotonic baseline for health timestamps
  init_baseline();

  #[cfg(debug_assertions)]
  EspLogger::initialize_default();
  // In release, bring up TCP syslog with its persistent spill buffer now — before WiFi —
  // so boot logs are captured and delivered once the server becomes reachable.
  #[cfg(not(debug_assertions))]
  log_storage::init_logging();

  log::info!("Starting...");

  let sys_loop = EspSystemEventLoop::take().unwrap();
  let timer_service = EspTimerService::new().unwrap();
  let nvs = EspDefaultNvsPartition::take().unwrap();
  let peripherals = Peripherals::take().unwrap();

  // Read and log the reset/wakeup reason and boot diagnostics BEFORE WiFi comes up. The
  // logger and its flash spill buffer are already running, so this record is captured and
  // delivered on the next connect even when WiFi itself is what failed.
  let reset_reason = ResetReason::get();
  log::info!("Reset reason: {reset_reason:?}");
  log::info!("Wakeup reason: {:?}", WakeupReason::get());
  match diagnostics::Diagnostics::new(nvs.clone()) {
    Ok(diag) => {
      let record = diag.record_boot(reset_reason);
      log::info!(
        "Boot #{} (previous reset reason discriminant: {:?})",
        record.boot_count,
        record.previous_reset,
      );
    }
    Err(err) => log::warn!("Boot diagnostics unavailable: {err:?}"),
  }

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

    // Start SNTP now that the network is up, so log timestamps reflect real time. Kept
    // alive for the lifetime of this scope; dropping it would stop syncing.
    #[cfg(not(debug_assertions))]
    let _sntp = match time_sync::start() {
      Ok(sntp) => Some(sntp),
      Err(err) => {
        log::error!("Failed to start SNTP: {err:?}");
        None
      }
    };

    // clear tickers
    LAST_ASYNC_TICK_1S_S.store(0, Ordering::Relaxed);
    LAST_TIMER_THREAD_TICK_S.store(0, Ordering::Relaxed);

    {
      // Backstop watchdog on its own std thread (independent of the async executor). Reboots
      // on either a stalled executor or a prolonged loss of MQTT connectivity. The Wi-Fi
      // supervisor normally restores the link long before the connectivity timeout fires;
      // this only triggers if reconnection is impossible (e.g. a dead AP backhaul).
      let connectivity_timeout_s = crate::config::CONFIG.wifi.connectivity_timeout.as_secs() as u32;
      std::thread::spawn(move || {
        loop {
          std::thread::sleep(std::time::Duration::from_secs(20));

          let now_s = now_secs();
          let last_s = LAST_ASYNC_TICK_1S_S.load(Ordering::Relaxed);
          if last_s != 0 && now_s.saturating_sub(last_s) > 20 {
            let age = |a: &AtomicU32| {
              let v = a.load(Ordering::Relaxed);
              if v == 0 { u32::MAX } else { now_s.saturating_sub(v) }
            };
            log::error!(
              "Async stalled >3m; ages async1s={}s timer_thread={}s",
              age(&LAST_ASYNC_TICK_1S_S),
              age(&LAST_TIMER_THREAD_TICK_S),
            );
            // wait to send log message before restarting
            std::thread::sleep(std::time::Duration::from_secs(1));
            esp_idf_svc::hal::reset::restart();
          }

          let disconnected_since = MQTT_DISCONNECTED_SINCE_S.load(Ordering::Relaxed);
          if disconnected_since != 0 && now_s.saturating_sub(disconnected_since) > connectivity_timeout_s {
            log::error!(
              "No MQTT connectivity for {}s; restarting",
              now_s.saturating_sub(disconnected_since),
            );
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
        let executor: &'static mut embassy_executor::Executor = Box::leak(Box::new(embassy_executor::Executor::new()));
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
        // Drive the 1s health ticker and the Wi-Fi reconnect supervisor concurrently. The
        // ticker never returns; the supervisor only returns (with an error) when reconnection
        // is hopeless, which bubbles up to a reboot below.
        let ticker = pin!(async {
          loop {
            embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
            LAST_ASYNC_TICK_1S_S.store(now_secs(), Ordering::Relaxed);
          }
          #[allow(unreachable_code)]
          Ok::<(), GarageError>(())
        });
        match select(ticker, pin!(wifi.supervise())).await {
          Either::First(result) | Either::Second(result) => result,
        }
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
