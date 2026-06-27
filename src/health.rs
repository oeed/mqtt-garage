use core::sync::atomic::{AtomicU32, Ordering};
use std::{sync::OnceLock, time::Instant};

// Baseline for monotonic seconds since init
static BASELINE: OnceLock<Instant> = OnceLock::new();

// Public tickers updated by various subsystems
pub static LAST_ASYNC_TICK_1S_S: AtomicU32 = AtomicU32::new(0);
/// Heartbeat updated by a separate Embassy executor running in its own std thread
pub static LAST_TIMER_THREAD_TICK_S: AtomicU32 = AtomicU32::new(0);

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
