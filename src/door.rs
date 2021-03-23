pub use config::DoorConfig;
pub use identifier::Identifier;

use self::{
  config::MqttConfig,
  remote::{DoorRemote, RemoteConfig},
  state::{State, TargetState},
  state_detector::StateDetector,
};
use crate::{error::GarageResult, mqtt_client::MqttClient};

mod command;
pub mod config;
pub mod identifier;
mod remote;
pub mod state;
mod state_detector;

#[derive(Debug)]
pub struct Door<'a, D: StateDetector> {
  identifier: Identifier,
  remote: DoorRemote,
  state_detector: D,
  current_state: State,
  target_state: TargetState,
  mqtt_client: &'a mut MqttClient,
  command_topic: String,
}

impl<'a, D: StateDetector> Door<'a, D> {
  pub async fn with_config(
    identifier: Identifier,
    state_detector: D::Config,
    remote: RemoteConfig,
    mqtt: MqttConfig,
    mqtt_client: &'a mut MqttClient,
  ) -> GarageResult<Door<'a, D>> {
    let mut state_detector = D::with_config(identifier.clone(), state_detector)?;
    let remote = DoorRemote::with_config(remote)?;

    let initial_state: State = state_detector.detect_state().into();

    let mut door = Door {
      identifier,
      remote,
      state_detector,
      // we initially assume the door is going to where it is meant to be going
      target_state: initial_state.end_state(),
      current_state: initial_state,
      command_topic: mqtt.command_topic,
      mqtt_client,
    };

    door.subscribe_commands().await?;

    Ok(door)
  }
}
