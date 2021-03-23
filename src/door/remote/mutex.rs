use std::sync::Mutex;

// TODO: using an actual mutex is probably overkill for this, although simpler than mucking with futures likely
/// A mutex to provide exclusive access to the radio waves of a remote.
/// Remotes use the same frequency, so two remotes emitting at the same time causes interference.
#[derive(Debug)]
pub struct RemoteMutex(Mutex<()>);

impl RemoteMutex {
  pub fn new() -> Self {
    RemoteMutex(Mutex::new(()))
  }

  pub fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
    self.0.lock().unwrap()
  }
}
