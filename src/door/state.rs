use std::{
  fmt,
  future::Future,
  pin::Pin,
  str::FromStr,
  task::{Context, Poll},
  time::{Duration, SystemTime},
};

use serde::Deserialize;
use tokio::time::{self, sleep, Sleep};

/// The state the door is trying to get to
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
  #[serde(rename = "OPEN")]
  Open,
  #[serde(rename = "CLOSED")]
  Closed,
}

impl TargetState {
  /// Get the travel state used to travel *to* this state
  pub(crate) fn from_travel_state(&self, travel: Travel) -> State {
    match self {
      TargetState::Open => State::Opening(travel),
      TargetState::Closed => State::Closing(travel),
    }
  }
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

/// Represents a currently occuring door travel
#[derive(Debug)]
pub struct Travel {
  /// Whether this travel was manually invoked
  pub is_manual: bool,
  pub(crate) expiry: Pin<Box<Sleep>>,
  /// The number of times this travel has been attempted, starting at 0
  attempt: u8,
  duration: Duration,
}

impl Travel {
  pub fn new(duration: Duration, is_manual: bool) -> Self {
    Travel {
      is_manual,
      expiry: Box::pin(time::sleep(duration)),
      duration,
      attempt: 0,
    }
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

#[derive(Debug)]
pub enum State {
  Opening(Travel),
  Open,
  Closing(Travel),
  Closed,
}

impl fmt::Display for State {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      State::Opening(_) => write!(f, "opening"),
      State::Open => write!(f, "open"),
      State::Closing(_) => write!(f, "closing"),
      State::Closed => write!(f, "closed"),
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
  pub fn travel_mut(&mut self) -> Option<&mut Travel> {
    match self {
      State::Opening(travel) | State::Closing(travel) => Some(travel),
      _ => None,
    }
  }

  /// Gets the target state this state will end up in (or is currently in)
  pub fn end_state(&self) -> TargetState {
    match self {
      State::Opening(..) | State::Open => TargetState::Open,
      State::Closing(..) | State::Closed => TargetState::Closed,
    }
  }

  /// Gets the target state this state started in before any transition
  pub fn start_state(&self) -> TargetState {
    match self {
      State::Opening(..) | State::Closed => TargetState::Closed,
      State::Closing(..) | State::Open => TargetState::Open,
    }
  }

  /// True if the state if opening or closing (i.e. in transition)
  pub fn is_travelling(&self) -> bool {
    match self {
      State::Opening(..) | State::Closing(..) => true,
      _ => false,
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
