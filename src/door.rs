use std::sync::Arc;

pub use config::DoorConfig;
pub use identifier::Identifier;
pub use remote::mutex::RemoteMutex;

use self::{
  remote::{DoorRemote, RemoteConfig},
  state::{State, TargetState},
  state_detector::StateDetector,
};
use crate::{
  error::GarageResult,
  mqtt_client::{MqttClient, PublishReceiver, PublishSender},
};

mod command;
pub mod config;
pub mod identifier;
mod remote;
pub mod state;
pub mod state_detector;

#[derive(Debug)]
pub struct Door<D: StateDetector> {
  identifier: Identifier,
  remote: DoorRemote,
  state_detector: D,
  current_state: State,
  target_state: TargetState,
  send_channel: PublishSender,
  command_topic: String,
  state_topic: String,
}

impl<D: StateDetector> Door<D> {
  pub async fn with_config(
    identifier: Identifier,
    command_topic: String,
    state_topic: String,
    initial_target_state: Option<TargetState>,
    remote: RemoteConfig,
    state_detector: D::Config,
    send_channel: PublishSender,
    remote_mutex: Arc<RemoteMutex>,
  ) -> GarageResult<Door<D>> {
    let remote = DoorRemote::with_config(remote, remote_mutex)?;
    let mut state_detector = D::with_config(identifier.clone(), state_detector)?;
    let initial_state: State = state_detector.detect_state().into();

    let mut door = Door {
      identifier,
      remote,
      state_detector,
      // we initially assume the door is going to where it is meant to be going
      target_state: initial_state.end_state(),
      current_state: initial_state,
      command_topic,
      state_topic,
      send_channel,
    };

    if let Some(target_state) = initial_target_state {
      door.to_target_state(target_state).await?;
    }

    Ok(door)
  }

  pub async fn listen(mut self, mut receive_channel: PublishReceiver) {
    loop {
      if let Some(publish) = receive_channel.recv().await {
        if &self.command_topic == &publish.topic {
          if let Ok(target_state) = toml::from_str(&publish.payload) {
            self.to_target_state(target_state).await.unwrap()
          }
        }
      }
      else {
        // channel ended
        return;
      }
    }
  }
}
