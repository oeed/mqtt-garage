use std::{future, pin::pin, str::FromStr};

use embassy_futures::select::{Either, Either3, select, select3};
use embassy_time::Timer;
use esp_idf_svc::{hal::gpio::Pins, mqtt::client::QoS};
use serde::Deserialize;

use self::{
  remote::DoorRemote,
  state::{SensorState, State, TargetState},
};
use crate::{
  config::CONFIG,
  door::state::{AssumedTravel, ConfirmedTravel},
  error::{GarageError, GarageResult},
  mqtt_client::{MqttChannels, MqttPublish, MqttTopicPublisher, MqttTopicReceiver},
};

pub mod remote;
pub mod state;

pub struct Door<'a> {
  publisher: MqttTopicPublisher<'a>,
  sensor_receiver: MqttTopicReceiver<'a, SensorPayload>,
  command_receiver: MqttTopicReceiver<'a, TargetState>,

  remote: DoorRemote,
  current_state: State,
}

#[derive(Debug, Deserialize)]
pub struct SensorPayload {
  /// `true` if closed
  contact: bool,
}

impl SensorPayload {
  pub fn into_state(self) -> SensorState {
    if self.contact {
      SensorState::Closed
    }
    else {
      SensorState::Open
    }
  }
}

impl<'a> Door<'a> {
  pub async fn new(pins: Pins, mqtt_channels: &'a MqttChannels) -> GarageResult<Door<'a>> {
    let remote = DoorRemote::new(pins)?;

    let sensor_receiver = mqtt_channels.sensor_receiver();

    log::info!("Getting initial state from sensor");

    let initial_state = select(
      pin!(async move { sensor_receiver.receive().await.into_state() }),
      pin!(Timer::after(embassy_time::Duration::from_secs(10))),
    )
    .await;

    let initial_state = match initial_state {
      Either::First(state) => state,
      Either::Second(_) => return Err(GarageError::DoorInitialisationTimeout),
    };
    log::info!("Initial state: {:?}", initial_state);

    let mut door = Door {
      publisher: mqtt_channels.publisher(),
      command_receiver: mqtt_channels.command_receiver(),
      sensor_receiver,
      current_state: initial_state.into(),
      remote,
    };

    door.publish_current_state().await;

    let initial_target_state =
      TargetState::from_str(&CONFIG.door.initial_target_state).expect("Invalid initial_target_state");
    door.goto_target_state(initial_target_state).await?;

    Ok(door)
  }

  pub async fn listen(mut self) -> GarageResult<()> {
    let mut next_target_state: Option<TargetState> = None;

    log::info!("Door listening with initial state: {:?}", self.current_state);
    // let result: GarageResult<()> =
    loop {
      // if there was a queued next state, and we're not travelling, move to it
      if let Some(target_state) = next_target_state
        && !self.current_state.is_travelling()
      {
        log::info!("Moving to state: {:?}", target_state);
        // only act on commands while not travelling
        next_target_state = None;
        self.goto_target_state(target_state).await?;
      }


      // determine what action is ready to be processed
      let action = select3(
        pin!(async {
          let payload = self.sensor_receiver.receive().await;
          payload.into_state()
        }),
        pin!(async {
          // wait for a state expiry to complete (e.g. assumedtravel time)
          if let Some(expiry) = self.current_state.expiry_mut() {
            expiry.await;
          }
          else {
            // if there's no expiry don't resolve this branch ever
            future::pending().await
          }
        }),
        pin!(async { self.command_receiver.receive().await }),
      )
      .await;

      log::info!("Action: {:?}", action);

      // process the action
      match action {
        Either3::First(detected_state) => {
          // detected state changed
          log::debug!(
            "Door detected state: {:?}, current state: {:?}",
            &detected_state,
            &self.current_state
          );

          match (&self.current_state, detected_state) {
            (State::Closed | State::AttemptingOpen(_), SensorState::Stuck) => {
              self.set_current_state(State::StuckClosed).await
            }
            (State::Open | State::Opening(_) | State::Closing(_), SensorState::Stuck) => {
              self.set_current_state(State::StuckOpen).await
            }
            (State::Closed | State::AttemptingOpen(_) | State::StuckClosed | State::StuckOpen, SensorState::Open) => {
              // door was stuck/closed but it's now open
              log::debug!("Door was opened");
              self
                .set_current_state(State::Opening(AssumedTravel::new(CONFIG.door.travel_duration)))
                .await
            }
            (
              State::Open | State::Closing(_) | State::StuckClosed | State::StuckOpen | State::Opening(_),
              SensorState::Closed,
            ) => {
              // door was open/stuck/closing and it's now closed
              log::debug!("Door was closed");
              self.set_current_state(State::Closed).await
            }
            _ => (), // no-op
          }
        }
        Either3::Second(()) => {
          // expiry resolved
          match &mut self.current_state {
            State::AttemptingOpen(confirmed_travel) | State::Closing(confirmed_travel) => {
              // the door didn't open/close as it was requested to
              if confirmed_travel.reattempt().is_ok() {
                // the travel expired, i.e. the door didn't move in to place before it should have
                // travel is still the current state at this point, so we can safely assume it hasn't completed

                // we're going to try again
                log::debug!("Door failed to move, triggering remote again");
                self.remote.trigger().await?;
              }
              else {
                // we've tried too many times
                log::info!("Door failed to move after maximum attemps, marking as stuck");
                match self.current_state {
                  State::AttemptingOpen(_) => self.set_current_state(State::StuckClosed).await,
                  State::Closing(_) => self.set_current_state(State::StuckClosed).await,
                  _ => unreachable!(),
                }
              }
            }
            State::Opening(_) => {
              // the assumed travel time has expired, mark it as being in the end state
              log::debug!("Door open travel assumed complete");
              self.set_current_state(State::Open).await;
            }
            State::Open | State::StuckOpen | State::Closed | State::StuckClosed => {
              unreachable!("state should not have an expiry")
            }
          }
        }
        Either3::Third(target_state) => {
          // command received
          log::info!("Next target state: {:?}", target_state);
          next_target_state = Some(target_state);
        }
      }
    }
  }

  async fn set_current_state(&mut self, current_state: State) {
    log::debug!("Door setting new state: {:?}", current_state);
    self.current_state = current_state;
    self.publish_current_state().await
  }

  async fn publish_current_state(&self) {
    self
      .publisher
      .publish(MqttPublish {
        topic: &CONFIG.door.state_topic,
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: self.current_state.as_str(),
      })
      .await;

    self
      .publisher
      .publish(MqttPublish {
        topic: &CONFIG.door.stuck_topic,
        qos: QoS::AtLeastOnce,
        retain: false,
        payload: self.current_state.stuck_state().as_str(),
      })
      .await;
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
          self
            .set_current_state(State::Closing(ConfirmedTravel::new(CONFIG.door.travel_duration)))
            .await;
        }
        TargetState::Open => {
          // we can detect if the door starts to open, so ensure it does
          self
            .set_current_state(State::AttemptingOpen(ConfirmedTravel::new(
              CONFIG.door.remote.max_latency_duration
                + CONFIG.door.remote.pressed_duration
                + CONFIG.door.remote.wait_duration,
            )))
            .await;
        }
      }
      // trigger the door
      log::debug!("Door is now targeting state {}, triggering remote", target_state);
      self.remote.trigger().await?;
    }

    Ok(())
  }
}
