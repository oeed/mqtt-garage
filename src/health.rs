use core::sync::atomic::{AtomicU32, Ordering};
use std::{sync::OnceLock, time::Instant};

// Baseline for monotonic seconds since init
static BASELINE: OnceLock<Instant> = OnceLock::new();

// Public tickers updated by various subsystems
pub static LAST_ASYNC_TICK_S: AtomicU32 = AtomicU32::new(0);
pub static LAST_ASYNC_TICK_1S_S: AtomicU32 = AtomicU32::new(0);
pub static LAST_MQTT_RX_TICK_S: AtomicU32 = AtomicU32::new(0);
pub static LAST_MQTT_TX_TICK_S: AtomicU32 = AtomicU32::new(0);
pub static LAST_DOOR_TICK_S: AtomicU32 = AtomicU32::new(0);
/// Heartbeat updated by a separate Embassy executor running in its own std thread
pub static LAST_TIMER_THREAD_TICK_S: AtomicU32 = AtomicU32::new(0);

// Section tracing (last polled/awaited external section)
pub static LAST_SECTION_ID: AtomicU32 = AtomicU32::new(0);
pub static LAST_SECTION_TS_S: AtomicU32 = AtomicU32::new(0);

// Known section identifiers
pub mod section {
  pub const MQTT_RX_NEXT: u32 = 1;
  pub const MQTT_RX_SENSOR_SEND: u32 = 2;
  pub const MQTT_RX_COMMAND_SEND: u32 = 3;
  pub const MQTT_TX_PUBLISH: u32 = 10;
  pub const MQTT_TX_SUBSCRIBE: u32 = 11;
  pub const DOOR_SENSOR_RECEIVE: u32 = 20;
  pub const DOOR_DEBOUNCE_TIMER: u32 = 21;
  pub const DOOR_EXPIRY_AWAIT: u32 = 22;
  pub const DOOR_COMMAND_RECEIVE: u32 = 23;
  pub const DOOR_REMOTE_TRIGGER: u32 = 24;
  pub const HEARTBEAT_TIMER_1S: u32 = 30;
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

pub fn mark_section(id: u32) {
  LAST_SECTION_ID.store(id, Ordering::Relaxed);
  LAST_SECTION_TS_S.store(now_secs(), Ordering::Relaxed);
}

pub fn last_section_age() -> u32 {
  age_secs(&LAST_SECTION_TS_S)
}

pub fn section_name(id: u32) -> &'static str {
  match id {
    section::MQTT_RX_NEXT => "MQTT_RX_NEXT",
    section::MQTT_RX_SENSOR_SEND => "MQTT_RX_SENSOR_SEND",
    section::MQTT_RX_COMMAND_SEND => "MQTT_RX_COMMAND_SEND",
    section::MQTT_TX_PUBLISH => "MQTT_TX_PUBLISH",
    section::MQTT_TX_SUBSCRIBE => "MQTT_TX_SUBSCRIBE",
    section::DOOR_SENSOR_RECEIVE => "DOOR_SENSOR_RECEIVE",
    section::DOOR_DEBOUNCE_TIMER => "DOOR_DEBOUNCE_TIMER",
    section::DOOR_EXPIRY_AWAIT => "DOOR_EXPIRY_AWAIT",
    section::DOOR_COMMAND_RECEIVE => "DOOR_COMMAND_RECEIVE",
    section::DOOR_REMOTE_TRIGGER => "DOOR_REMOTE_TRIGGER",
    section::HEARTBEAT_TIMER_1S => "HEARTBEAT_TIMER_1S",
    _ => "UNKNOWN",
  }
}
