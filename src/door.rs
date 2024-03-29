use std::{fmt, str::FromStr, sync::Arc, time::Duration};
use chrono::{TimeZone, Utc, Timelike};
pub use config::DoorConfig;
pub use identifier::Identifier;
use log::{debug, info};
pub use remote::mutex::RemoteMutex;
use tokio::{sync::Mutex, time::sleep};

use self::{
  remote::{DoorRemote, RemoteConfig},
  state::{State, Stuck, TargetState},
  state_detector::StateDetector,
};
use crate::{
  error::GarageResult,
  mqtt_client::{receiver::MqttReceiver, sender::PublishSender},
};

mod command;
pub mod config;
pub mod identifier;
mod remote;
pub mod state;
pub mod state_detector;

#[derive(Debug)]
pub struct Door<D: StateDetector + Send> {
  identifier: Identifier,
  remote: DoorRemote,
  state_detector: D,
  current_state: State,
  target_state: TargetState,
  send_channel: PublishSender,
  command_topic: String,
  state_topic: String,
  stuck_topic: Option<String>,
  stuck: Stuck,
}

impl<D: StateDetector + Send> fmt::Display for Door<D> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Door ({})", self.identifier.0)
  }
}

impl<D: StateDetector + Send> Door<D> {
  pub async fn with_config(
    identifier: Identifier,
    command_topic: String,
    state_topic: String,
    stuck_topic: Option<String>,
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
      stuck_topic,
      send_channel,
      stuck: Stuck::Ok,
    };

    door.set_current_state(initial_state).await?;

    if let Some(target_state) = initial_target_state {
      door.to_target_state(target_state).await?;
    }

    Ok(door)
  }
}

impl<D: StateDetector + Send + 'static> Door<D> {
  pub async fn listen(mut self, receiver: &mut MqttReceiver) -> GarageResult<()> {
    info!("{} initialised", &self);
    let mut door_receive_channel = self.subscribe(receiver).await?;
    let state_detector_receive_channel = self.state_detector.subscribe(receiver).await?;

    let should_check = self.state_detector.should_check();
    let command_topic = self.command_topic.clone();
    let mutex = Arc::new(Mutex::new(self));

    if should_check {
      let mutex = Arc::clone(&mutex);
      tokio::spawn(async move {
        // concurrently check if the door's state has changed
        loop {
          sleep(Duration::from_secs(2)).await;
          mutex.lock().await.check_state().await.unwrap();
        }
      });
    }

    if let Some(mut state_detector_receive_channel) = state_detector_receive_channel {
      let mutex = Arc::clone(&mutex);
      tokio::spawn(async move {
        loop {
          if let Some(publish) = state_detector_receive_channel.recv().await {
            let mut door = mutex.lock().await;
            door.state_detector.receive_message(publish);
          }
          else {
            // channel ended
            return;
          }
        }
      });
    }

    tokio::spawn(async move {
      loop {
        if let Some(publish) = door_receive_channel.recv().await {
          if &command_topic == &publish.topic {
            if let Ok(target_state) = TargetState::from_str(&publish.payload) {
              let mut door = mutex.lock().await;
              debug!("{} got told to moved to state: {:?}", &door, &target_state);
              door.to_target_state(target_state).await.unwrap()
            }
          }
        }
        else {
          // channel ended
          return;
        }
      }
    });

    Ok(())
  }
}
