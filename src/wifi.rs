use embedded_svc::wifi::{self, AuthMethod, Configuration};
use esp_idf_svc::{
  eventloop::EspSystemEventLoop,
  hal::modem::Modem,
  ipv4::{self, DHCPClientSettings},
  netif::{EspNetif, NetifConfiguration, NetifStack},
  nvs::EspDefaultNvsPartition,
  timer::EspTaskTimerService,
  wifi::{AsyncWifi, EspWifi, WifiDriver},
};
use smart_leds::colors;

use crate::{
  config::CONFIG,
  error::{GarageError, GarageResult},
  rgb::RgbLed,
};

#[must_use]
pub struct Wifi {
  wifi: AsyncWifi<EspWifi<'static>>,
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
      sys_loop,
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

    rgb_led.on(colors::RED);

    log::info!("Wifi starting...");
    wifi.start().await?;
    log::info!("Wifi connecting to {}", CONFIG.wifi.ssid);

    wifi.connect().await?;
    log::info!("Wifi connected");

    wifi.wait_netif_up().await?;
    log::info!("Wifi netif up");
    rgb_led.off();

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    log::info!("Wifi DHCP info: {ip_info:?}");

    Ok(Wifi { wifi })
  }

  pub async fn wait_for_disconnect(mut self) -> GarageResult<()> {
    self.wifi.wifi_wait(|wifi| wifi.is_up(), None).await?;
    log::info!("Wifi disconnected");
    Err(GarageError::WifiDisconnected)
  }
}
