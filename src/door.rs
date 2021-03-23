use self::{
  mqtt::{DoorMqtt, MqttConfig},
  remote::{DoorRemote, RemoteConfig},
  state_detector::StateDetector,
};
use crate::error::GarageResult;
pub use config::DoorConfig;
pub use identifier::Identifier;

pub mod config;
pub mod identifier;
mod mqtt;
mod remote;
pub mod state;
mod state_detector;

#[derive(Debug)]
pub struct Door<D: StateDetector> {
  identifier: Identifier,
  mqtt: DoorMqtt,
  remote: DoorRemote,
  state_detector: D,
}

impl<D: StateDetector> Door<D> {
  pub fn with_config(
    identifier: Identifier,
    topic_name: String,
    state_detector: D::Config,
    remote: RemoteConfig,
    mqtt: MqttConfig,
  ) -> GarageResult<Self> {
    let state_detector = D::with_config(identifier.clone(), state_detector)?;
    let remote = DoorRemote::with_config(remote)?;
    let mqtt = DoorMqtt::with_config(mqtt)?;

    Ok(Door {
      identifier,
      mqtt,
      remote,
      state_detector,
    })
  }
}
