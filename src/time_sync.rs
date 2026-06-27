//! Wall-clock time via SNTP.
//!
//! The ESP32 has no battery-backed RTC, so absolute time is unknown until SNTP syncs. On
//! every sync we update the syslog logger's wall-clock anchor, which lets buffered and
//! recovered log records be stamped with the real time they occurred (see `syslog_esp32::clock`).

use core::time::Duration;

use esp_idf_svc::sntp::{EspSntp, OperatingMode, SntpConf, SyncMode};

use crate::config::CONFIG;
use crate::error::GarageResult;

/// Start SNTP against the configured server. The returned handle must be kept alive for
/// syncing to continue; dropping it stops SNTP.
pub fn start() -> GarageResult<EspSntp<'static>> {
  let mut conf = SntpConf {
    operating_mode: OperatingMode::Poll,
    sync_mode: SyncMode::Immediate,
    ..Default::default()
  };
  // Point every server slot at the configured source so a LAN-only NTP server is honoured
  // without falling back to the public pool. CONFIG is static, so these borrows are 'static.
  for server in conf.servers.iter_mut() {
    *server = CONFIG.wifi.ntp_server.as_ref();
  }

  let sntp = EspSntp::new_with_callback(&conf, |synced: Duration| {
    // `synced` is the current time as a duration since the Unix epoch.
    syslog_esp32::sync_wall_clock(synced.as_micros() as u64);
  })?;

  Ok(sntp)
}
