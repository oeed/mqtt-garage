pub use config::DoorConfig;
pub use identifier::Identifier;
pub use remote::mutex::RemoteMutex;

use self::{
  remote::{DoorRemote, RemoteConfig},
  state::{State, TargetState},
  state_detector::StateDetector,
};
use crate::{error::GarageResult, mqtt_client::MqttClient};

mod command;
pub mod concrete;
pub mod config;
pub mod identifier;
mod remote;
pub mod state;
mod state_detector;

#[derive(Debug)]
pub struct Door<'a, D: StateDetector + Send> {
  identifier: Identifier,
  remote: DoorRemote<'a>,
  state_detector: D,
  current_state: State,
  target_state: TargetState,
  mqtt_client: &'a mut MqttClient,
  command_topic: String,
  state_topic: String,
}

impl<'a, D: StateDetector + Send> Door<'a, D> {
  pub async fn with_config(
    identifier: Identifier,
    command_topic: String,
    state_topic: String,
    initial_target_state: Option<TargetState>,
    state_detector: D::Config,
    remote: RemoteConfig,
    mqtt_client: &'a mut MqttClient,
    remote_mutex: &'a RemoteMutex,
  ) -> GarageResult<Door<'a, D>> {
    let mut state_detector = D::with_config(identifier.clone(), state_detector)?;
    let remote = DoorRemote::with_config(remote, remote_mutex)?;

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
      mqtt_client,
    };

    if let Some(target_state) = initial_target_state {
      door.to_target_state(target_state).await?;
    }

    door.subscribe_commands().await?;

    Ok(door)
  }
}
