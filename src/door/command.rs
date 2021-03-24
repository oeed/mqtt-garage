use super::{state_detector::StateDetector, Door};
use crate::{
  error::GarageResult,
  mqtt_client::{MqttClient, PublishReceiver},
};


/// Detecting open/close commands and acting upon them
impl<D: StateDetector> Door<D> {
  pub async fn subscribe(&mut self, mqtt_client: &mut MqttClient) -> GarageResult<PublishReceiver> {
    mqtt_client
      .subscribe(self.command_topic.clone(), rumqttc::QoS::AtLeastOnce)
      .await
  }

  pub fn is_message_for_door(&self, topic: &str) -> bool {
    &self.command_topic == topic
  }
}
