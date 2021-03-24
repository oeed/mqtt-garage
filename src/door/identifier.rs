use serde::Serialize;

/// An identifier for a door.
///
/// Defined by the door's configuration key, i.e. [doors.identifier-here]
///
/// Also used to save the door's last state.
#[derive(Debug, Serialize, Hash, PartialEq, Eq, Clone)]
pub struct Identifier(String);

impl From<String> for Identifier {
  fn from(string: String) -> Self {
    Identifier(string)
  }
}
