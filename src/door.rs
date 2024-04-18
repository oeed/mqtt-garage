use std::sync::Arc;

use tokio::sync::mpsc;

use self::{
  config::DoorConfig,
  controller::{config::DoorControllerConfig, remote::mutex::RemoteMutex, DoorController},
  detector::DoorDetector,
  identifier::Identifier,
};
use crate::{
  error::GarageResult,
  mqtt_client::{receiver::MqttReceiver, MqttPublish},
};

pub mod config;
pub mod controller;
pub mod detector;
pub mod identifier;
pub mod state;

pub struct Door<D: DoorDetector> {
  pub identifier: Identifier,
  detector: D,
  // we cannot initialise the controller until after the MQTT receiver starts running
  controller_mqtt_tx: mpsc::UnboundedSender<MqttPublish>,
  controller_mqtt_rx: mpsc::UnboundedReceiver<MqttPublish>,
  controller_config: DoorControllerConfig,
  remote_mutex: Arc<RemoteMutex>,
}

impl<D: DoorDetector> Door<D> {
  pub async fn new(
    identifier: Identifier,
    door_config: DoorConfig<D>,
    controller_mqtt_tx: mpsc::UnboundedSender<MqttPublish>,
    remote_mutex: Arc<RemoteMutex>,
    mqtt_receiver: &mut MqttReceiver,
  ) -> GarageResult<Self> {
    let detector = D::new(identifier.clone(), door_config.detector, mqtt_receiver).await?;

    let controller_mqtt_rx = mqtt_receiver
      .subscribe(door_config.controller.command_topic.clone(), rumqttc::QoS::AtLeastOnce)
      .await?;

    Ok(Door {
      identifier,
      detector,
      controller_mqtt_tx,
      controller_mqtt_rx,
      controller_config: door_config.controller,
      remote_mutex,
    })
  }

  pub async fn listen(self) -> GarageResult<()> {
    let (initial_state, detector_rx) = self.detector.listen().await?;

    let controller = DoorController::new(
      self.identifier,
      self.controller_config,
      self.controller_mqtt_tx,
      self.controller_mqtt_rx,
      self.remote_mutex,
      initial_state.into(),
    )?;
    controller.listen(detector_rx).await?;

    Ok(())
  }
}
