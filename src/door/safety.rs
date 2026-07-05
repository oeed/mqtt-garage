use esp_idf_svc::hal::gpio::{self, Gpio13, PinDriver};

use crate::error::GarageResult;

/// Drives the relay wired across the opener's safety input (PE ↔ GND).
///
/// The relay uses its **normally-closed** contact: while de-energized, PE is shorted to GND and the
/// opener is permitted to close. Energizing the relay opens that contact, which the opener reads as an
/// obstructed safety input and refuses to close.
///
/// The pin is active-high, matching [`crate::door::remote::DoorRemote`] — driving it high energizes the
/// relay. So:
/// - **low**  → relay de-energized → PE↔GND closed → closing permitted   (safe to close)
/// - **high** → relay energized    → PE↔GND open   → closing inhibited    (not safe to close)
///
/// De-energized is also the power-on/crash default: if the ESP is unpowered the NC contact stays
/// closed, so the opener can still close under its own control.
pub struct SafetyRelay {
  pin: PinDriver<'static, Gpio13, gpio::Output>,
}

impl SafetyRelay {
  pub fn new(gpio: Gpio13) -> GarageResult<Self> {
    let mut pin = PinDriver::output(gpio)?;
    // Start de-energized: NC contact closed, closing permitted.
    pin.set_low()?;
    Ok(SafetyRelay { pin })
  }

  /// Reflect the current safe-to-close status onto the opener's safety input.
  pub fn set_safe_to_close(&mut self, safe_to_close: bool) -> GarageResult<()> {
    if safe_to_close {
      // Permit closing: de-energize so the NC contact shorts PE↔GND.
      self.pin.set_low()?;
    }
    else {
      // Inhibit closing: energize to open the NC contact.
      self.pin.set_high()?;
    }
    Ok(())
  }
}
