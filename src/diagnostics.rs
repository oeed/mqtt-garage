//! Small reboot-surviving diagnostic counters in NVS.
//!
//! The spill-buffered syslog already preserves individual log lines (including Wi-Fi
//! disconnect reason codes) across reboots, so this only tracks the coarse counters that are
//! useful even if those logs are lost: how many times the device has booted and why it last
//! reset. Both are read and logged on the next boot.

use esp_idf_svc::{
  hal::reset::ResetReason,
  nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault},
  sys::EspError,
};

const NAMESPACE: &str = "diag";
const KEY_BOOT_COUNT: &str = "boot_count";
const KEY_LAST_RESET: &str = "last_reset";

/// Snapshot of the diagnostic state recorded at boot.
pub struct BootRecord {
  /// Boot counter after incrementing for this boot.
  pub boot_count: u32,
  /// Reset reason discriminant persisted on the *previous* boot, if any.
  pub previous_reset: Option<u32>,
}

pub struct Diagnostics {
  nvs: EspNvs<NvsDefault>,
}

impl Diagnostics {
  pub fn new(partition: EspDefaultNvsPartition) -> Result<Diagnostics, EspError> {
    Ok(Diagnostics {
      nvs: EspNvs::new(partition, NAMESPACE, true)?,
    })
  }

  /// Increment the boot counter and persist this boot's reset reason, returning the new count
  /// and the reason stored on the previous boot. Best-effort: NVS errors are swallowed so a
  /// flash hiccup can never block start-up.
  pub fn record_boot(&self, reset: ResetReason) -> BootRecord {
    let boot_count = self
      .nvs
      .get_u32(KEY_BOOT_COUNT)
      .ok()
      .flatten()
      .unwrap_or(0)
      .wrapping_add(1);
    let previous_reset = self.nvs.get_u32(KEY_LAST_RESET).ok().flatten();

    let _ = self.nvs.set_u32(KEY_BOOT_COUNT, boot_count);
    let _ = self.nvs.set_u32(KEY_LAST_RESET, reset as u32);

    BootRecord {
      boot_count,
      previous_reset,
    }
  }
}
