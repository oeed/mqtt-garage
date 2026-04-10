use std::{fmt, pin::Pin, str::FromStr};

use embassy_time::Timer;

use crate::{config::CONFIG, door::SensorPayload};

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

/// Represents a door travel where we can confirm the door has reached the target state.
// #[derive(Debug)]
pub struct ConfirmedTravel {
  pub(crate) expiry: Pin<Box<Timer>>,
  /// The number of times this travel has been attempted, starting at 0
  attempt: u8,
  duration: embassy_time::Duration,
}

impl ConfirmedTravel {
  pub fn new(duration: embassy_time::Duration) -> Self {
    ConfirmedTravel {
      expiry: Box::pin(Timer::after(duration)),
      duration,
      attempt: 0,
    }
  }

  pub fn expiry_mut(&mut self) -> &mut Pin<Box<Timer>> {
    &mut self.expiry
  }

  /// Renew the expiry on this travel an increment the attempt counter.
  ///
  /// Returns `Err(())` if greater than the maximum number of attempts.
  pub fn reattempt(&mut self) -> Result<(), ()> {
    if self.attempt >= CONFIG.door.max_attempts {
      Err(())
    }
    else {
      // Use travel_duration for reattempts since the door may need to complete
      // a full travel. The short initial duration is only for detecting that
      // movement started on the first attempt.
      let reattempt_duration = CONFIG.door.travel_duration;
      self.expiry = Box::pin(Timer::after(reattempt_duration));
      self.attempt += 1;
      log::info!(
        "Door travel reattempt {} of {} (duration: {:?})",
        self.attempt,
        CONFIG.door.max_attempts,
        reattempt_duration
      );
      Ok(())
    }
  }
}

pub enum State {
  AttemptingOpen(ConfirmedTravel),
  AttemptingClose(ConfirmedTravel),
  /// We have to assume when the door finished opening
  Opening(ConfirmedTravel),
  Open,
  StuckOpen,
  /// We can confirm when the door closes
  Closing(ConfirmedTravel),
  Closed,
  StuckClosed,
}

impl State {
  pub fn as_str(&self) -> &'static str {
    match self {
      State::AttemptingOpen(_) | State::Opening(_) => "opening",
      State::Open | State::StuckOpen => "open",
      State::AttemptingClose(_) | State::Closing(_) => "closing",
      State::Closed | State::StuckClosed => "closed",
    }
  }
}

impl fmt::Debug for State {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      State::AttemptingOpen(_) => write!(f, "AttemptingOpen"),
      State::AttemptingClose(_) => write!(f, "AttemptingClose"),
      State::Opening(_) => write!(f, "Opening"),
      State::Open => write!(f, "Open"),
      State::StuckOpen => write!(f, "StuckOpen"),
      State::Closing(_) => write!(f, "Closing"),
      State::Closed => write!(f, "Closed"),
      State::StuckClosed => write!(f, "StuckClosed"),
    }
  }
}

impl From<SensorState> for State {
  fn from(target_state: SensorState) -> Self {
    match target_state {
      SensorState::Open => State::Open,
      SensorState::Closed => State::Closed,
      SensorState::Stuck => State::Open,
      SensorState::Moving => State::Open,
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
  pub fn confirmed_travel_mut(&mut self) -> Option<&mut ConfirmedTravel> {
    match self {
      State::AttemptingOpen(travel)
      | State::Opening(travel)
      | State::AttemptingClose(travel)
      | State::Closing(travel) => Some(travel),
      _ => None,
    }
  }

  pub fn expiry_mut(&mut self) -> Option<&mut Pin<Box<Timer>>> {
    match self {
      State::AttemptingOpen(travel)
      | State::Opening(travel)
      | State::AttemptingClose(travel)
      | State::Closing(travel) => Some(travel.expiry_mut()),
      _ => None,
    }
  }

  /// True if the state if opening or closing (i.e. in transition)
  pub fn is_travelling(&self) -> bool {
    match self {
      State::AttemptingOpen(..) | State::Opening(..) | State::AttemptingClose(..) | State::Closing(..) => true,
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
///
/// It can also determine if the door is likely stuck.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SensorState {
  Open,
  Closed,
  Moving,
  /// Used for invalid payload too
  Stuck,
}

impl SensorState {
  pub fn from_sensors(open_sensor: SensorPayload, closed_sensor: SensorPayload) -> Self {
    match (open_sensor.contact, closed_sensor.contact) {
      (true, true) => SensorState::Stuck,
      (true, false) => SensorState::Open,
      (false, true) => SensorState::Closed,
      (false, false) => SensorState::Moving,
    }
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
