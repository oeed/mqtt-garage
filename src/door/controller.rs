use std::{fmt, str::FromStr, sync::Arc, time::Duration};

use rumqttc::QoS;
use tokio::{
  select,
  sync::mpsc::{self, UnboundedReceiver},
};

use self::{
  config::DoorControllerConfig,
  remote::{mutex::RemoteMutex, DoorRemote},
};
use super::{
  identifier::Identifier,
  state::{DetectedState, State, Stuck, TargetState},
};
use crate::{
  door::state::Travel,
  error::GarageResult,
  mqtt_client::{sender::PublishSender, MqttPublish},
};

pub mod config;
pub mod remote;

const MAX_STUCK_REATTEMPTS: u8 = 5;

#[derive(Debug)]
pub struct DoorController {
  identifier: Identifier,
  remote: DoorRemote,
  current_state: State,
  target_state: TargetState,
  mqtt_tx: PublishSender,
  command_topic: String,
  state_topic: String,
  stuck_topic: Option<String>,
  stuck: Stuck,
  travel_duration: Duration,
  mqtt_rx: UnboundedReceiver<MqttPublish>,
}

impl fmt::Display for DoorController {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DoorController ({})", self.identifier.0)
  }
}

impl DoorController {
  pub fn new(
    identifier: Identifier,
    config: DoorControllerConfig,
    mqtt_tx: mpsc::UnboundedSender<MqttPublish>,
    mqtt_rx: UnboundedReceiver<MqttPublish>,
    remote_mutex: Arc<RemoteMutex>,
    initial_state: State,
  ) -> GarageResult<DoorController> {
    let remote = DoorRemote::new(config.remote, remote_mutex)?;

    let controller = DoorController {
      identifier,
      target_state: initial_state.end_state(),
      current_state: initial_state,
      command_topic: config.command_topic,
      state_topic: config.state_topic,
      stuck_topic: config.stuck_topic,
      travel_duration: config.travel_duration,
      mqtt_tx,
      stuck: Stuck::Ok,
      remote,
      mqtt_rx,
    };

    Ok(controller)
  }

  pub async fn listen(mut self, mut detector_rx: mpsc::UnboundedReceiver<DetectedState>) -> GarageResult<()> {
    log::info!("{} listening with initial state: {:?}", &self, self.current_state);
    tokio::spawn(async move {
      loop {
        let _: GarageResult<()> = select! {
          Some(detected_state) = detector_rx.recv() => {
            // detected state changed
            log::debug!("{} detected state: {:?}, current state: {:?}", &self, &detected_state, &self.current_state);
            if detected_state == DetectedState::Stuck {
              self.set_stuck(Stuck::Stuck);
            }
            else {
              self.set_stuck(Stuck::Ok);
            }

            match (&self.current_state, detected_state ) {
              (State::Closed, DetectedState::Open) => {
                // door was closed but it's now open (i.e. manually opened)
                log::debug!("{} state manually changed to: {:?}", &self, &detected_state);
                self.target_state = TargetState::Open;
                self.set_current_state(State::Opening(Travel::new(self.travel_duration,true)));
              }
              (State::Open, DetectedState::Closed) => {
                // door was open but it's now closed (i.e. manually closed)
                log::debug!("{} state manually changed to: {:?}", &self, &detected_state);
                self.target_state = TargetState::Closed;
                self.set_current_state(State::Closed);
              }
              (State::Opening(_), DetectedState::Open) => {
                // TODO: this will result in doors instantly showing as opened, it needs to wait the travel time. toggle on is_manual
                // door was opening and it's now open
                log::debug!("{} finished opening", &self);
                self.set_current_state(State::Open);
              }
              (State::Closing(_), DetectedState::Closed) => {
                // door was opening and it's now open
                log::debug!("{} finished opening", &self);
                self.set_current_state(State::Closed);
              }
              (_, DetectedState::Stuck) |(State::Opening(_), DetectedState::Closed)|(State::Open, DetectedState::Open) |
              (State::Closing(_), DetectedState::Open) |
              (State::Closed, DetectedState::Closed)  => {
                // no-op; either it's in the wrong state while travelling and will be reattempted, or it's in the right state
              }
            }

            Ok(())
          },

          Some(travel) = async {
            if let Some(travel) = self.current_state.travel_mut() {
              (&mut travel.expiry).await;
              Some(travel)
            }else {
              None
            }
          } => {
            // the travel expired, i.e. the door didn't move in to place before it should have
            // travel is still the current state at this point, so we can safely assume it hasn't completed

            if travel.is_manual {
              // the manually induced travel probably finished now, so mark it as such
              log::debug!("{} manual travel assumed complete", &self);
              self.set_current_state(self.current_state.end_state().into());
            }
            else if travel.reattempt(MAX_STUCK_REATTEMPTS).is_ok() {
              // we're going to try again
              log::debug!("{} door failed to move, triggering remote again", &self);
              self.remote.trigger().await;
            } else {
              // we've tried too many times
              self.set_stuck(Stuck::Stuck);
            }

            Ok(())
          }

          Some(publish) = self.mqtt_rx.recv() => {
            if &self.command_topic == &publish.topic {
              if let Ok(target_state) = TargetState::from_str(&publish.payload) {
                // commanded to move to `target_state`
                log::debug!("{} was commanded to moved to state: {:?}, current state: {:?}", &self, &target_state, &self.current_state);
                // TODO: what if the door is currently moving?
                self.set_target_state(target_state).await;
              }
            }

            Ok(())
          }

        };
      }
    });

    // TODO: run this somewhere
    // if let Some(target_state) = initial_target_state {
    //   door.to_target_state(target_state).await?;
    // }
    Ok(())
  }

  fn set_stuck(&mut self, stuck: Stuck) {
    if self.stuck != stuck {
      self.stuck = stuck;
      if let Some(stuck_topic) = &self.stuck_topic {
        self
          .mqtt_tx
          .send(MqttPublish {
            topic: stuck_topic.clone(),
            qos: QoS::AtLeastOnce,
            retain: false,
            payload: stuck.to_string(),
          })
          .expect("MQTT channel closed");
      }
    }
  }

  fn set_current_state(&mut self, current_state: State) {
    log::debug!("{} setting new state: {:?}", &self, current_state);
    let payload = current_state.to_string();
    self.current_state = current_state;
    self
      .mqtt_tx
      .send(MqttPublish {
        topic: self.state_topic.clone(),
        qos: QoS::AtLeastOnce,
        retain: true,
        payload,
      })
      .expect("MQTT channel closed");
  }

  async fn set_target_state(&mut self, target_state: TargetState) {
    if self.current_state != target_state {
      self.target_state = target_state;
      // we're not in our target state, transition to travelling and trigger the door
      self.set_current_state(
        self
          .target_state
          .from_travel_state(Travel::new(self.travel_duration, false)),
      );

      // trigger the door
      log::debug!("{} is now targeting state {}, triggering remote", &self, target_state);
      self.remote.trigger().await;
    }
  }
}
