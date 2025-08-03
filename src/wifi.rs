use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_svc::{
  eventloop::EspSystemEventLoop,
  hal::modem::Modem,
  nvs::EspDefaultNvsPartition,
  timer::EspTaskTimerService,
  wifi::{AsyncWifi, EspWifi},
};

use crate::{config::CONFIG, error::GarageResult};

#[must_use]
pub struct Wifi {
  _wifi: AsyncWifi<EspWifi<'static>>,
}

impl Wifi {
  pub async fn connect(
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    timer_service: EspTaskTimerService,
    nvs: EspDefaultNvsPartition,
  ) -> GarageResult<Wifi> {
    let mut wifi = AsyncWifi::wrap(
      EspWifi::new(modem, sys_loop.clone(), Some(nvs))?,
      sys_loop,
      timer_service,
    )?;

    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
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

    wifi.start().await?;
    log::info!("Wifi connecting to {}", CONFIG.wifi.ssid);

    wifi.connect().await?;
    log::info!("Wifi connected");

    wifi.wait_netif_up().await?;
    log::info!("Wifi netif up");

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    log::info!("Wifi DHCP info: {ip_info:?}");

    // TODO: probably need to monitor the connection status and reconnect if needed?
    Ok(Wifi { _wifi: wifi })
  }
}
