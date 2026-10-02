use embassy_time::{Duration, Instant, Ticker, Timer};
use esp_idf_svc::hal::gpio::{self, AnyIOPin, Gpio4, Gpio5, IOPin, PinDriver, Pull};

use crate::{config::CONFIG, door::state::SensorState, error::GarageResult};

/// How often the contact sensors are sampled. Far shorter than the debounce window, so it adds
/// nothing noticeable to how quickly a change is recognised.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Time for the inputs to reach their idle level once the pull-ups are enabled, before the first
/// reading is trusted. The input filter capacitor charges through the pull-up — a few milliseconds
/// through the external one, ~20ms through the internal one alone — and until it has, an open switch
/// reads as contact.
const SETTLE_DURATION: Duration = Duration::from_millis(50);

/// A wired reed switch between a GPIO and GND that closes while its magnet is alongside it (e.g. a
/// security alarm reed switch).
///
/// The pin is pulled up, so the switch closing under its magnet pulls it low. A cut or disconnected
/// wire therefore reads as *no contact* — the door simply isn't believed to be at that sensor — and
/// can never fabricate a position.
struct ContactSensor {
  pin: PinDriver<'static, AnyIOPin, gpio::Input>,
}

impl ContactSensor {
  fn new(pin: AnyIOPin) -> GarageResult<Self> {
    let mut pin = PinDriver::input(pin)?;
    // The internal pull-up (~45kΩ) is too weak to rely on alone over a long run past a motor, but it
    // costs nothing alongside the external one and keeps the input defined if that is ever missing.
    pin.set_pull(Pull::Up)?;
    Ok(ContactSensor { pin })
  }

  fn in_contact(&self) -> bool {
    self.pin.is_low()
  }
}

/// The two contact sensors at either end of the door's travel, debounced together into a single
/// [`SensorState`].
pub struct DoorSensors {
  /// Sensor at the top of the travel; on contact, the door is open.
  open: ContactSensor,
  /// Sensor at the bottom of the travel; on contact, the door is closed.
  closed: ContactSensor,
  /// The last reading that held for the whole debounce window.
  state: SensorState,
  /// A different reading, and when it was first seen, while it waits out the debounce window.
  pending: Option<(SensorState, Instant)>,
}

impl DoorSensors {
  pub async fn new(open_gpio: Gpio4, closed_gpio: Gpio5) -> GarageResult<Self> {
    let open = ContactSensor::new(open_gpio.downgrade())?;
    let closed = ContactSensor::new(closed_gpio.downgrade())?;
    Timer::after(SETTLE_DURATION).await;
    let state = SensorState::from_contacts(open.in_contact(), closed.in_contact());
    Ok(DoorSensors {
      open,
      closed,
      state,
      pending: None,
    })
  }

  /// The current debounced reading.
  pub fn state(&self) -> SensorState {
    self.state
  }

  /// Wait until the reading changes and the change has held for the debounce window, then return it.
  ///
  /// A reading that reverts within the window is discarded, so a reed switch chattering as the door
  /// passes it, or a marginal magnet blipping while stationary, is never acted on. Safe to cancel: any
  /// change still being debounced is kept and resumed on the next call.
  pub async fn next_change(&mut self) -> SensorState {
    let mut ticker = Ticker::every(POLL_INTERVAL);
    loop {
      ticker.next().await;

      let raw = SensorState::from_contacts(self.open.in_contact(), self.closed.in_contact());
      if raw == self.state {
        self.pending = None;
        continue;
      }

      match self.pending {
        Some((candidate, since)) if candidate == raw => {
          if since.elapsed() >= CONFIG.door.debounce_duration {
            self.pending = None;
            self.state = raw;
            return raw;
          }
        }
        // A new candidate, or a further change to the one already pending: restart the window.
        _ => self.pending = Some((raw, Instant::now())),
      }
    }
  }
}
