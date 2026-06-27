use core::sync::atomic::Ordering;

use embassy_time::{Duration, Timer, with_timeout};
use embedded_svc::wifi::{self, AuthMethod, Configuration};
use esp_idf_svc::{
  eventloop::{EspSystemEventLoop, EspSystemSubscription},
  hal::modem::Modem,
  ipv4::{self, DHCPClientSettings},
  netif::{EspNetif, NetifConfiguration, NetifStack},
  nvs::EspDefaultNvsPartition,
  timer::EspTaskTimerService,
  wifi::{AsyncWifi, EspWifi, WifiDriver, WifiEvent},
};
use smart_leds::colors;

use crate::{
  config::CONFIG,
  error::{GarageError, GarageResult},
  health::LAST_WIFI_DISCONNECT_REASON,
  rgb::RgbLed,
};

#[must_use]
pub struct Wifi {
  wifi: AsyncWifi<EspWifi<'static>>,
  // Kept alive for the lifetime of the connection so the disconnect-reason callback keeps
  // firing; dropping this handle unsubscribes from the event loop.
  _disconnect_sub: EspSystemSubscription<'static>,
}

impl Wifi {
  pub async fn connect(
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    timer_service: EspTaskTimerService,
    nvs: EspDefaultNvsPartition,
    rgb_led: &mut RgbLed,
  ) -> GarageResult<Wifi> {
    let driver = WifiDriver::new(modem, sys_loop.clone(), Some(nvs))?;
    let netif_config = NetifConfiguration {
      ip_configuration: Some(ipv4::Configuration::Client(ipv4::ClientConfiguration::DHCP(
        DHCPClientSettings {
          hostname: Some(CONFIG.wifi.hostname.as_ref().try_into().unwrap()),
        },
      ))),

      ..NetifConfiguration::wifi_default_client()
    };
    let mut wifi = AsyncWifi::wrap(
      EspWifi::wrap_all(
        driver,
        EspNetif::new_with_conf(&netif_config)?,
        EspNetif::new(NetifStack::Ap)?,
      )?,
      sys_loop.clone(),
      timer_service,
    )?;

    let wifi_configuration: Configuration = Configuration::Client(wifi::ClientConfiguration {
      ssid: CONFIG.wifi.ssid.as_ref().try_into().unwrap(),
      bssid: None,
      auth_method: if CONFIG.wifi.password.is_empty() {
        AuthMethod::None
      }
      else {
        AuthMethod::WPA2Personal
      },
      password: CONFIG.wifi.password.as_ref().try_into().unwrap(),
      channel: None,
      ..Default::default()
    });

    wifi.set_configuration(&wifi_configuration)?;

    // Record the reason code of every dropped association so the cause is visible in syslog
    // after the next reconnect, even when the drop itself happened while offline.
    let disconnect_sub = sys_loop.subscribe::<WifiEvent<'_>, _>(move |event: WifiEvent<'_>| {
      if let WifiEvent::StaDisconnected(data) = event {
        LAST_WIFI_DISCONNECT_REASON.store(u32::from(data.reason()), Ordering::Relaxed);
      }
    })?;

    rgb_led.on(colors::RED);

    log::info!("Wifi starting...");
    // `start()` is the one bring-up call esp-idf-svc does not bound internally, so cap it.
    with_timeout(CONFIG.wifi.connect_timeout, wifi.start())
      .await
      .map_err(|_| GarageError::WifiTimeout)??;

    log::info!("Wifi connecting to {}", CONFIG.wifi.ssid);
    associate_with_backoff(&mut wifi).await?;
    log::info!("Wifi connected");
    rgb_led.off();

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    log::info!("Wifi DHCP info: {ip_info:?}");

    // The syslog logger is initialized in `log_storage` before WiFi starts, so its worker
    // thread will connect and drain any logs spilled during boot now that the link is up.

    Ok(Wifi {
      wifi,
      _disconnect_sub: disconnect_sub,
    })
  }

  /// Watch the link and actively re-associate whenever it drops. esp-idf-svc does not
  /// reconnect on its own, so without this a mid-session disconnect leaves the device
  /// associated to nothing until a power cycle. Runs for the lifetime of the program; only
  /// returns (with an error) if reconnection keeps failing, letting `main` reboot.
  pub async fn supervise(mut self) -> GarageResult<()> {
    loop {
      // Block until the interface is no longer up. `wifi_wait` re-evaluates the matcher on
      // every Wi-Fi event, so a `StaDisconnected` wakes it promptly.
      self.wifi.wifi_wait(|wifi| wifi.is_up(), None).await?;

      let reason = LAST_WIFI_DISCONNECT_REASON.load(Ordering::Relaxed);
      log::warn!("Wifi link down (last disconnect reason code {reason}); reconnecting");

      associate_with_backoff(&mut self.wifi).await?;
      log::info!("Wifi reconnected");
    }
  }
}

/// A single bounded association attempt: associate, then wait for a DHCP lease. Each step is
/// wrapped so neither can block longer than the configured timeout.
async fn associate_once(wifi: &mut AsyncWifi<EspWifi<'static>>) -> GarageResult<()> {
  with_timeout(CONFIG.wifi.connect_timeout, wifi.connect())
    .await
    .map_err(|_| GarageError::WifiTimeout)??;
  with_timeout(CONFIG.wifi.connect_timeout, wifi.wait_netif_up())
    .await
    .map_err(|_| GarageError::WifiTimeout)??;
  Ok(())
}

/// Retry [`associate_once`] with exponential backoff. Returns an error once
/// `connect_max_attempts` consecutive attempts have failed so the caller can reboot rather
/// than spin forever.
async fn associate_with_backoff(wifi: &mut AsyncWifi<EspWifi<'static>>) -> GarageResult<()> {
  let mut backoff = CONFIG.wifi.reconnect_backoff;
  for attempt in 1..=CONFIG.wifi.connect_max_attempts {
    match associate_once(wifi).await {
      Ok(()) => return Ok(()),
      Err(err) => {
        log::warn!(
          "Wifi association attempt {}/{} failed: {err:?}",
          attempt,
          CONFIG.wifi.connect_max_attempts
        );
        if attempt < CONFIG.wifi.connect_max_attempts {
          Timer::after(backoff).await;
          backoff = core::cmp::min(
            Duration::from_ticks(backoff.as_ticks().saturating_mul(2)),
            CONFIG.wifi.reconnect_max_backoff,
          );
        }
      }
    }
  }
  Err(GarageError::WifiConnectFailed)
}
