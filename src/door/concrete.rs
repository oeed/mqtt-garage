use super::{
  state_detector::{assumed::AssumedStateDetector, sensor::SensorStateDetector, StateDetectorConfig},
  Door, DoorConfig, Identifier, RemoteMutex,
};
use crate::{error::GarageResult, mqtt_client::MqttClient};


/// Provides a door with a concrete, set sized, type
pub enum ConcreteDoor<'a> {
  SensorDoor(Door<'a, SensorStateDetector>),
  AssumedDoor(Door<'a, AssumedStateDetector>),
}

impl<'a> ConcreteDoor<'a> {
  pub async fn with_config(
    identifier: Identifier,
    config: DoorConfig,
    mqtt_client: &'a mut MqttClient,
    remote_mutex: &'a RemoteMutex,
  ) -> GarageResult<ConcreteDoor<'a>> {
    match config.state_detector {
      StateDetectorConfig::Assumed(state_detector) => Ok(ConcreteDoor::AssumedDoor(
        Door::with_config(
          identifier,
          config.command_topic,
          config.state_topic,
          config.initial_target_state,
          state_detector,
          config.remote,
          mqtt_client,
          remote_mutex,
        )
        .await?,
      )),
      StateDetectorConfig::Sensor(state_detector) => Ok(ConcreteDoor::SensorDoor(
        Door::with_config(
          identifier,
          config.command_topic,
          config.state_topic,
          config.initial_target_state,
          state_detector,
          config.remote,
          mqtt_client,
          remote_mutex,
        )
        .await?,
      )),
    }
  }
}
