use serde::Deserialize;

use super::{state::TargetState, state_detector::StateDetector, Door};
use crate::error::GarageResult;

#[derive(Debug, Deserialize)]
enum Command {
  #[serde(rename = "OPEN")]
  Open,
  #[serde(rename = "CLOSE")]
  Close,
}

/// Detecting open/close commands and acting upon them
impl<'a, D: StateDetector> Door<'a, D> {
  pub async fn subscribe_commands(&mut self) -> GarageResult<()> {
    self
      .mqtt_client
      .subscribe(self.command_topic.clone(), rumqttc::QoS::AtLeastOnce)
      .await?;
    Ok(())
  }

  async fn on_message(&mut self, topic: &str, command: String) -> GarageResult<()> {
    if &self.command_topic == topic {
      if let Ok(command) = toml::from_str(&command) {
        match command {
          Command::Open => self.to_target_state(TargetState::Open).await,
          Command::Close => self.to_target_state(TargetState::Closed).await,
        }
      }
      else {
        Ok(())
      }
    }
    else {
      Ok(())
    }
  }
}
