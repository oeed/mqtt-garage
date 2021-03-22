pub enum DoorState {
  Opening,
  Open,
  Closing,
  Closed,
  Stuck,
}

/// The state the door is trying to get to
#[derive(Debug, Clone, Copy)]
pub enum TargetState {
  Open,
  Closed,
}
