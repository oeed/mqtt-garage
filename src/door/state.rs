use std::{fmt, pin::Pin, str::FromStr};

use embassy_time::Timer;

use crate::config::CONFIG;

/// The state the door is trying to get to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
  Open,
  Closed,
}

impl FromStr for TargetState {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "OPEN" => Ok(TargetState::Open),
      "CLOSED" => Ok(TargetState::Closed),
      _ => Err(()),
    }
  }
}

impl fmt::Display for TargetState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TargetState::Open => write!(f, "OPEN"),
      TargetState::Closed => write!(f, "CLOSED"),
    }
  }
}

/// A command received over MQTT to act on the door.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorCommand {
  /// Move the door toward a target state (open/close), subject to the safe-to-close gate.
  Target(TargetState),
  /// Pulse the remote relay directly, bypassing all door state and the safe-to-close gate.
  /// Intended for debugging/testing: it just presses the button, exactly like a handheld remote.
  Trigger,
}

impl PartialEq<TargetState> for State {
  fn eq(&self, other: &TargetState) -> bool {
    match (self, other) {
      (State::Open, TargetState::Open) | (State::Closed, TargetState::Closed) => true,
      _ => false,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stuck {
  Ok,
  Stuck,
}

impl Stuck {
  pub fn as_str(&self) -> &'static str {
    match self {
      Stuck::Ok => "ok",
      Stuck::Stuck => "stuck",
    }
  }
}

/// Why the door is believed to be moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelOrigin {
  /// We pulsed the relay ourselves in response to a command.
  Commanded,
  /// The sensors showed movement we did not ask for — a handheld remote, the wall button, or the
  /// door being pushed. The relay is never actuated on this path.
  Observed,
}

/// A door movement in progress, budgeted to last until the door should have reached the far sensor.
///
/// The deadline is deliberately anchored to *arrival* rather than *departure*: a door that never
/// leaves its origin sensor and one that stops part-way both look the same until the far sensor has
/// had its full chance, and at expiry the live sensor reading says which it was.
pub struct Travel {
  expiry: Pin<Box<Timer>>,
  origin: TravelOrigin,
}

impl Travel {
  /// A travel we initiated by pulsing the relay: budget for the opener reacting, the press itself,
  /// and then the full traverse.
  pub fn commanded() -> Self {
    Travel::new(
      TravelOrigin::Commanded,
      CONFIG.door.remote.max_latency_duration
        + CONFIG.door.remote.pressed_duration
        + CONFIG.door.remote.wait_duration
        + CONFIG.door.travel_duration,
    )
  }

  /// A travel we noticed from the sensors, already underway by the time it was debounced.
  pub fn observed() -> Self {
    Travel::new(TravelOrigin::Observed, CONFIG.door.travel_duration)
  }

  fn new(origin: TravelOrigin, duration: embassy_time::Duration) -> Self {
    Travel {
      // Arrival is only recognised after the debounce window, so budget for it on top of the traverse
      // itself — otherwise a door that reaches its sensor right at the end of the window would be
      // declared unconfirmed a moment before the confirmation lands.
      expiry: Box::pin(Timer::after(duration + CONFIG.door.debounce_duration)),
      origin,
    }
  }

  pub fn expiry_mut(&mut self) -> &mut Pin<Box<Timer>> {
    &mut self.expiry
  }

  pub fn origin(&self) -> TravelOrigin {
    self.origin
  }
}

pub enum State {
  Opening(Travel),
  Open,
  /// Believed open, but unverified — either a travel finished with the door at neither sensor, or the
  /// sensors are contradicting each other. Always re-pulses on the next command.
  StuckOpen,
  Closing(Travel),
  Closed,
  /// Believed closed, but with the sensors contradicting each other.
  StuckClosed,
}

impl State {
  pub fn as_str(&self) -> &'static str {
    match self {
      State::Opening(_) => "opening",
      State::Open | State::StuckOpen => "open",
      State::Closing(_) => "closing",
      State::Closed | State::StuckClosed => "closed",
    }
  }
}

impl fmt::Debug for State {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      State::Opening(travel) => write!(f, "Opening({:?})", travel.origin()),
      State::Open => write!(f, "Open"),
      State::StuckOpen => write!(f, "StuckOpen"),
      State::Closing(travel) => write!(f, "Closing({:?})", travel.origin()),
      State::Closed => write!(f, "Closed"),
      State::StuckClosed => write!(f, "StuckClosed"),
    }
  }
}

impl From<SensorState> for State {
  fn from(sensor_state: SensorState) -> Self {
    match sensor_state {
      SensorState::Open => State::Open,
      // Neither a mid-travel reading nor two contradicting sensors tells us where the door actually is,
      // and an unverified door is reported open so it is never mistaken for a secured one.
      SensorState::Conflict | SensorState::Moving => State::StuckOpen,
      SensorState::Closed => State::Closed,
    }
  }
}

impl From<TargetState> for State {
  fn from(target_state: TargetState) -> Self {
    match target_state {
      TargetState::Open => State::Open,
      TargetState::Closed => State::Closed,
    }
  }
}

impl State {
  pub fn expiry_mut(&mut self) -> Option<&mut Pin<Box<Timer>>> {
    match self {
      State::Opening(travel) | State::Closing(travel) => Some(travel.expiry_mut()),
      _ => None,
    }
  }

  /// True if the state if opening or closing (i.e. in transition)
  pub fn is_travelling(&self) -> bool {
    match self {
      State::Opening(..) | State::Closing(..) => true,
      _ => false,
    }
  }

  pub fn stuck_state(&self) -> Stuck {
    match self {
      State::StuckOpen | State::StuckClosed => Stuck::Stuck,
      _ => Stuck::Ok,
    }
  }
}

/// Detectors can tell if a door is open or closed, but not where along it is.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SensorState {
  Open,
  Closed,
  /// Neither sensor is in contact: the door is somewhere between the two.
  Moving,
  /// Both sensors report contact. The door cannot be at the top and the bottom at once, so one of
  /// the two is faulty — a shorted cable, or a switch picking up a magnet it shouldn't.
  Conflict,
}

impl SensorState {
  pub fn from_contacts(open_contact: bool, closed_contact: bool) -> Self {
    match (open_contact, closed_contact) {
      (true, true) => SensorState::Conflict,
      (true, false) => SensorState::Open,
      (false, true) => SensorState::Closed,
      (false, false) => SensorState::Moving,
    }
  }

  /// Whether the open sensor is in contact.
  pub fn open_contact(&self) -> bool {
    matches!(self, SensorState::Open | SensorState::Conflict)
  }

  /// Whether the closed sensor is in contact.
  pub fn closed_contact(&self) -> bool {
    matches!(self, SensorState::Closed | SensorState::Conflict)
  }
}


impl FromStr for SensorState {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "OPEN" => Ok(SensorState::Open),
      "CLOSED" => Ok(SensorState::Closed),
      _ => Err(()),
    }
  }
}


impl From<TargetState> for SensorState {
  fn from(target_state: TargetState) -> Self {
    match target_state {
      TargetState::Open => SensorState::Open,
      TargetState::Closed => SensorState::Closed,
    }
  }
}
