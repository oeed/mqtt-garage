//! Persistent storage and initialization for the syslog logger.
//!
//! Mounts a FAT (wear-levelled) filesystem on the `logspill` flash partition and registers
//! the global logger. Logging is initialized before WiFi comes up, so logs produced during
//! boot and any connectivity outage are spilled to flash and drained to the syslog server
//! once it becomes reachable — surviving reboots in the meantime.

use std::ffi::CString;

use esp_idf_svc::sys::{esp_vfs_fat_mount_config_t, esp_vfs_fat_spiflash_mount_rw_wl, wl_handle_t};

use crate::config::CONFIG;

/// VFS mount point for the spill filesystem (`std::fs` paths live under here).
const SPILL_BASE_PATH: &str = "/spill";
/// File the syslog logger writes its spill buffer to.
const SPILL_FILE_PATH: &str = "/spill/log.bin";
/// Partition label, matching `partitions.csv`.
const SPILL_PARTITION_LABEL: &str = "logspill";
/// Cap on buffered records before the oldest are dropped (a few hundred lines is plenty).
const MAX_SPILL_RECORDS: usize = 500;

/// Mount the spill filesystem, then register the global TCP/spill syslog logger.
///
/// Best-effort: if the filesystem can't be mounted the logger still runs (without
/// cross-reboot persistence), and if the logger can't be registered we fall back to
/// serial-only logging so the device is never left mute.
pub fn init_logging() {
  let mounted = mount().is_ok();

  use log::LevelFilter;
  use syslog_esp32::{Facility, init_tcp_ipv4};

  let result = init_tcp_ipv4(
    Some(CONFIG.wifi.hostname.as_ref()),
    "mqtt-garage",
    Facility::LOG_USER,
    LevelFilter::Info,
    CONFIG.wifi.syslog_server.into(),
    SPILL_FILE_PATH,
    MAX_SPILL_RECORDS,
  );

  match result {
    Ok(()) => {
      if !mounted {
        log::warn!("Log spill filesystem not mounted; logs will not persist across reboots");
      }
    }
    Err(err) => {
      esp_idf_svc::log::EspLogger::initialize_default();
      log::error!("Failed to initialize syslog logger, falling back to serial: {err:?}");
    }
  }
}

/// Mount the FAT+wear-levelling filesystem on the spill partition, formatting it on first
/// use (or if the existing volume is unreadable).
fn mount() -> Result<(), esp_idf_svc::sys::EspError> {
  let base_path = CString::new(SPILL_BASE_PATH).unwrap();
  let label = CString::new(SPILL_PARTITION_LABEL).unwrap();

  let mount_config = esp_vfs_fat_mount_config_t {
    format_if_mount_failed: true,
    max_files: 4,
    allocation_unit_size: 4096,
    disk_status_check_enable: false,
  };

  // -1 is WL_INVALID_HANDLE; the call fills it in on success.
  let mut wl_handle: wl_handle_t = -1;

  esp_idf_svc::sys::esp!(unsafe {
    esp_vfs_fat_spiflash_mount_rw_wl(base_path.as_ptr(), label.as_ptr(), &mount_config, &mut wl_handle)
  })
}
