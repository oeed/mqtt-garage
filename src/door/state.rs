use std::{fmt, pin::Pin, str::FromStr, time::Duration};

use serde::Deserialize;
use tokio::time::{self, Sleep};

/// The state the door is trying to get to
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
  #[serde(rename = "OPEN")]
  Open,
  #[serde(rename = "CLOSED")]
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

impl fmt::Display for Stuck {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Stuck::Ok => write!(f, "ok"),
      Stuck::Stuck => write!(f, "stuck"),
    }
  }
}

/// Represents a door travel where we can confirm the door has reached the target state.
#[derive(Debug)]
pub struct ConfirmedTravel {
  pub(crate) expiry: Pin<Box<Sleep>>,
  /// The number of times this travel has been attempted, starting at 0
  attempt: u8,
  duration: Duration,
}

impl ConfirmedTravel {
  pub fn new(duration: Duration) -> Self {
    ConfirmedTravel {
      expiry: Box::pin(time::sleep(duration)),
      duration,
      attempt: 0,
    }
  }

  pub fn expiry_mut(&mut self) -> &mut Pin<Box<Sleep>> {
    &mut self.expiry
  }

  /// Renew the expiry on this travel an increment the attempt counter.
  ///
  /// Returns `Err(())` if greater than the maximum number of attempts.
  pub fn reattempt(&mut self, max_attempts: u8) -> Result<(), ()> {
    if self.attempt >= max_attempts {
      Err(())
    }
    else {
      self.expiry = Box::pin(time::sleep(self.duration));
      self.attempt += 1;
      Ok(())
    }
  }
}

/// Represents an assumed travel. Once complete we assume the door to be in the target state.
#[derive(Debug)]
pub struct AssumedTravel {
  pub(crate) expiry: Pin<Box<Sleep>>,
}

impl AssumedTravel {
  pub fn new(duration: Duration) -> Self {
    AssumedTravel {
      expiry: Box::pin(time::sleep(duration)),
    }
  }

  pub fn expiry_mut(&mut self) -> &mut Pin<Box<Sleep>> {
    &mut self.expiry
  }
}

pub enum State {
  AttemptingOpen(ConfirmedTravel),
  /// We have to assume when the door finished opening
  Opening(AssumedTravel),
  Open,
  StuckOpen,
  /// We can confirm when the door closes
  Closing(ConfirmedTravel),
  Closed,
  StuckClosed,
}

impl fmt::Display for State {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      State::AttemptingOpen(_) | State::Opening(_) => write!(f, "opening"),
      State::Open | State::StuckOpen => write!(f, "open"),
      State::Closing(_) => write!(f, "closing"),
      State::Closed | State::StuckClosed => write!(f, "closed"),
    }
  }
}

impl fmt::Debug for State {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      State::AttemptingOpen(_) => write!(f, "AttemptingOpen"),
      State::Opening(_) => write!(f, "Opening"),
      State::Open => write!(f, "Open"),
      State::StuckOpen => write!(f, "StuckOpen"),
      State::Closing(_) => write!(f, "Closing"),
      State::Closed => write!(f, "Closed"),
      State::StuckClosed => write!(f, "StuckClosed"),
    }
  }
}

impl From<DetectedState> for State {
  fn from(target_state: DetectedState) -> Self {
    match target_state {
      DetectedState::Open => State::Open,
      DetectedState::Closed => State::Closed,
      DetectedState::Stuck => State::Open,
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
      State::AttemptingOpen(travel) | State::Closing(travel) => Some(travel),
      _ => None,
    }
  }

  pub fn assumed_travel_mut(&mut self) -> Option<&mut AssumedTravel> {
    match self {
      State::Opening(travel) => Some(travel),
      _ => None,
    }
  }

  pub fn expiry_mut(&mut self) -> Option<&mut Pin<Box<Sleep>>> {
    match self {
      State::Opening(travel) => Some(travel.expiry_mut()),
      State::AttemptingOpen(travel) | State::Closing(travel) => Some(travel.expiry_mut()),
      _ => None,
    }
  }

  /// True if the state if opening or closing (i.e. in transition)
  pub fn is_travelling(&self) -> bool {
    match self {
      State::Opening(..) | State::AttemptingOpen(..) | State::Closing(..) => true,
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

/// Detectors can tell if a door is open or closed, but not where long it is.
///
/// It can also determine if the door is likely stuck.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DetectedState {
  Open,
  Closed,
  Stuck,
}

impl From<TargetState> for DetectedState {
  fn from(target_state: TargetState) -> Self {
    match target_state {
      TargetState::Open => DetectedState::Open,
      TargetState::Closed => DetectedState::Closed,
    }
  }
}
