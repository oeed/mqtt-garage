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
  state::{DetectedState, State, TargetState},
};
use crate::{
  door::state::{AssumedTravel, ConfirmedTravel},
  error::{GarageError, GarageResult},
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
  mqtt_tx: PublishSender,
  command_topic: String,
  state_topic: String,
  stuck_topic: Option<String>,
  initial_target_state: Option<TargetState>,
  travel_duration: Duration,
  max_remote_latency_duration: Duration,
  mqtt_rx: UnboundedReceiver<MqttPublish>,
}

impl fmt::Display for DoorController {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "DoorController ({})", self.identifier.0)
  }
}

impl DoorController {
  pub async fn new(
    identifier: Identifier,
    config: DoorControllerConfig,
    mqtt_tx: mpsc::UnboundedSender<MqttPublish>,
    mqtt_rx: UnboundedReceiver<MqttPublish>,
    remote_mutex: Arc<RemoteMutex>,
    initial_state: State,
  ) -> GarageResult<DoorController> {
    let remote = DoorRemote::new(config.remote, remote_mutex)?;

    let mut controller = DoorController {
      identifier,
      current_state: initial_state,
      command_topic: config.command_topic,
      state_topic: config.state_topic,
      stuck_topic: config.stuck_topic,
      travel_duration: config.travel_duration,
      initial_target_state: config.initial_target_state,
      max_remote_latency_duration: config.max_remote_latency_duration,
      mqtt_tx,
      remote,
      mqtt_rx,
    };

    controller.publish_current_state()?;

    if let Some(target_state) = controller.initial_target_state {
      controller.goto_target_state(target_state).await?;
    }

    Ok(controller)
  }

  pub async fn listen(mut self, mut detector_rx: mpsc::UnboundedReceiver<DetectedState>) -> GarageResult<()> {
    let mut next_target_state: Option<TargetState> = None;

    log::info!("{} listening with initial state: {:?}", &self, self.current_state);
    loop {
      let result: GarageResult<()> = select! {
        Some(detected_state) = detector_rx.recv() => {
          // detected state changed
          log::debug!("{} detected state: {:?}, current state: {:?}", &self, &detected_state, &self.current_state);

          match (&self.current_state, detected_state) {
            (State::Closed | State::AttemptingOpen(_), DetectedState::Stuck) => {
              self.set_current_state(State::StuckClosed)
            }
            (State::Open | State::Opening(_) | State::Closing(_), DetectedState::Stuck) => {
              self.set_current_state(State::StuckOpen)
            }
            (State::Closed | State::AttemptingOpen(_)| State::StuckClosed | State::StuckOpen, DetectedState::Open) => {
              // door was stuck/closed but it's now open
              log::debug!("{} was opened", &self);
              self.set_current_state(State::Opening(AssumedTravel::new(self.travel_duration)),)
            }
            (State::Open | State::Closing(_) | State::StuckClosed | State::StuckOpen | State::Opening(_), DetectedState::Closed) => {
              // door was open/stuck/closing and it's now closed
              log::debug!("{} was closed", &self);
              self.set_current_state(State::Closed)
            }
            _ => Ok(()) // no-op
          }
        },

        Some(_) = async {
          if let Some(expiry) = self.current_state.expiry_mut() {
            expiry.await;
            Some(())
          } else {
            None
          }
        } => {
          match &mut self.current_state {
            State::AttemptingOpen(confirmed_travel) | State::Closing(confirmed_travel) => {
              // the door didn't open/close as it was requested to
              if confirmed_travel.reattempt(MAX_STUCK_REATTEMPTS).is_ok() {
                // the travel expired, i.e. the door didn't move in to place before it should have
                // travel is still the current state at this point, so we can safely assume it hasn't completed

                // we're going to try again
                log::debug!("{} door failed to move, triggering remote again", &self);
                self.remote.trigger().await;
              } else {
                // we've tried too many times
                log::debug!("{} door failed to move after maximum attemps, marking as stuck", &self);
                match self.current_state {
                  State::AttemptingOpen(_) => self.set_current_state(State::StuckClosed)?,
                  State::Closing(_) => self.set_current_state(State::StuckClosed)?,
                  _ => unreachable!(),
                }
              }
            },
            State::Opening(_) => {
              // the assumed travel time has expired, mark it as being in the end state
              log::debug!("{} open travel assumed complete", &self);
              self.set_current_state(State::Open)?;
            },
            State::Open | State::StuckOpen | State::Closed | State::StuckClosed => unreachable!("state should not have an expiry"),
          }

          Ok(())
        }

        Some(target_state) = async { next_target_state }, if !self.current_state.is_travelling() => { // only act on commands while not travelling
          next_target_state = None;
          // commanded to move to `target_state`
          log::debug!("{} was commanded to moved to state: {:?}, current state: {:?}", &self, &target_state, &self.current_state);
          self.goto_target_state(target_state).await
        }

        Some(publish) = self.mqtt_rx.recv() => {
          if &self.command_topic == &publish.topic {
            if let Ok(target_state) = TargetState::from_str(&publish.payload) {
              next_target_state = Some(target_state);
            }
          }

          Ok(())
        }

        else => {
          log::error!("{} listener ended (channels closed, MQTT connection likely lost)", &self);
          break Err(GarageError::MqttClosed);
        }
      };

      if let Err(err) = result {
        return Err::<(), _>(err);
      }
    }
  }

  fn set_current_state(&mut self, current_state: State) -> GarageResult<()> {
    log::debug!("{} setting new state: {:?}", &self, current_state);
    self.current_state = current_state;
    self.publish_current_state()
  }

  fn publish_current_state(&self) -> GarageResult<()> {
    self
      .mqtt_tx
      .send(MqttPublish {
        topic: self.state_topic.clone(),
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: self.current_state.to_string(),
      })
      .map_err(|_| GarageError::MqttClosed)?;

    if let Some(stuck_topic) = &self.stuck_topic {
      self
        .mqtt_tx
        .send(MqttPublish {
          topic: stuck_topic.clone(),
          qos: QoS::AtLeastOnce,
          retain: false,
          payload: self.current_state.stuck_state().to_string(),
        })
        .map_err(|_| GarageError::MqttClosed)?;
    }

    Ok(())
  }

  async fn goto_target_state(&mut self, target_state: TargetState) -> GarageResult<()> {
    if self.current_state.is_travelling() {
      panic!("Door is currently travelling, cannot move to another target state");
    }
    else if self.current_state != target_state {
      // we're not in our target state, transition to travelling and trigger the door
      match target_state {
        TargetState::Closed => {
          // because we can't be for sure if the door actually moves from the open state, we assume it's closing
          self.set_current_state(State::Closing(ConfirmedTravel::new(self.travel_duration)))?;
        }
        TargetState::Open => {
          // we can detect if the door starts to open, so ensure it does
          self.set_current_state(State::AttemptingOpen(ConfirmedTravel::new(
            self.max_remote_latency_duration,
          )))?;
        }
      }
      // trigger the door
      log::debug!("{} is now targeting state {}, triggering remote", &self, target_state);
      self.remote.trigger().await;
    }

    Ok(())
  }
}
