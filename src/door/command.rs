use futures::Future;
use serde::Deserialize;

use super::{concrete::ConcreteDoor, state::TargetState, state_detector::StateDetector, Door};
use crate::error::GarageResult;

#[derive(Debug, Deserialize)]
enum Command {
  #[serde(rename = "OPEN")]
  Open,
  #[serde(rename = "CLOSE")]
  Close,
}

/// Detecting open/close commands and acting upon them
impl<'a, D: StateDetector + Send> Door<'a, D> {
  pub async fn subscribe_commands(&mut self) -> GarageResult<()> {
    self
      .mqtt_client
      .subscribe(self.command_topic.clone(), rumqttc::QoS::AtLeastOnce)
      .await?;
    Ok(())
  }

  pub async fn on_message(&mut self, topic: &str, command: &str) -> GarageResult<()> {
    if &self.command_topic == topic {
      if let Ok(command) = toml::from_str(&command) {
        match command {
          Command::Open => self.to_target_state(TargetState::Open).await?,
          Command::Close => self.to_target_state(TargetState::Closed).await?,
        }
      }
    }

    Ok(())
  }
}

impl<'a> ConcreteDoor<'a> {
  pub async fn on_message(&mut self, topic: &str, command: &str) -> GarageResult<()> {
    match self {
      ConcreteDoor::AssumedDoor(door) => door.on_message(topic, command).await,
      ConcreteDoor::SensorDoor(door) => door.on_message(topic, command).await,
    }
  }
}
