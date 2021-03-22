use serde::{Deserialize, Serialize};

/// An identifier for a door.
///
/// Defined by the door's configuration key, i.e. [doors.identifier-here]
///
/// Also used to save the door's last state.
#[derive(Debug, Deserialize, Serialize, Hash, PartialEq, Eq, Clone)]
pub struct Identifier(String);
