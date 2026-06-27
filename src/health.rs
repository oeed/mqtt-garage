use core::sync::atomic::{AtomicU32, Ordering};
use std::{sync::OnceLock, time::Instant};

// Baseline for monotonic seconds since init
static BASELINE: OnceLock<Instant> = OnceLock::new();

// Public tickers updated by various subsystems
pub static LAST_ASYNC_TICK_1S_S: AtomicU32 = AtomicU32::new(0);
/// Heartbeat updated by a separate Embassy executor running in its own std thread
pub static LAST_TIMER_THREAD_TICK_S: AtomicU32 = AtomicU32::new(0);

/// Monotonic second at which MQTT entered (and has since stayed in) the Disconnected state.
/// 0 means currently connected, or not yet connected for the first time — in both cases the
/// connectivity watchdog stays disarmed so it never reboots during normal boot-up.
pub static MQTT_DISCONNECTED_SINCE_S: AtomicU32 = AtomicU32::new(0);

/// Last `WifiEvent::StaDisconnected` reason code seen (0 = none). Recorded from the Wi-Fi
/// event callback so the cause of a drop is known on the next successful connect.
pub static LAST_WIFI_DISCONNECT_REASON: AtomicU32 = AtomicU32::new(0);

/// Mark MQTT as connected: disarm the connectivity watchdog.
pub fn mark_mqtt_connected() {
  MQTT_DISCONNECTED_SINCE_S.store(0, Ordering::Relaxed);
}

/// Mark MQTT as disconnected: arm the connectivity watchdog at the first transition, keeping
/// the original disconnect time so the timeout measures the whole outage.
pub fn mark_mqtt_disconnected() {
  let now = now_secs().max(1);
  let _ = MQTT_DISCONNECTED_SINCE_S.compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
}

pub fn init_baseline() {
  let _ = BASELINE.set(Instant::now());
}

pub fn now_secs() -> u32 {
  BASELINE.get().map(|t| t.elapsed().as_secs() as u32).unwrap_or(0)
}

pub fn age_secs(last: &AtomicU32) -> u32 {
  let now = now_secs();
  let last_s = last.load(Ordering::Relaxed);
  if last_s == 0 {
    u32::MAX
  }
  else {
    now.saturating_sub(last_s)
  }
}
