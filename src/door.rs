use std::{future, pin::pin, str::FromStr};

use embassy_futures::select::{Either, Either4, select, select4};
use embassy_time::Timer;
use esp_idf_svc::{hal::gpio::Gpio14, mqtt::client::QoS};
use serde::Deserialize;
use smart_leds::colors;

use self::{
  remote::DoorRemote,
  state::{SensorState, State, TargetState},
};
use crate::{
  config::CONFIG,
  door::state::ConfirmedTravel,
  error::{GarageError, GarageResult},
  mqtt_client::{MqttChannels, MqttPublish, MqttTopicPublisher, MqttTopicReceiver},
  rgb::RgbLed,
};

pub mod remote;
pub mod state;

pub struct Door<'a> {
  publisher: MqttTopicPublisher<'a>,
  /// Sensor for the top (i.e. on contact, the door is open)
  open_sensor_receiver: MqttTopicReceiver<'a, SensorPayload>,
  /// Sensor for the bottom (i.e. on contact, the door is closed)
  closed_sensor_receiver: MqttTopicReceiver<'a, SensorPayload>,
  command_receiver: MqttTopicReceiver<'a, TargetState>,
  safe_to_close_receiver: MqttTopicReceiver<'a, bool>,

  last_open_sensor: SensorPayload,
  last_closed_sensor: SensorPayload,
  /// Whether it is currently safe to close the door. Defaults to `true` until told otherwise.
  safe_to_close: bool,

  remote: DoorRemote<'a>,
  current_state: State,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct SensorPayload {
  /// `true` if closed
  contact: bool,
}

impl<'a> Door<'a> {
  pub async fn new(gpio: Gpio14, mqtt_channels: &'a MqttChannels, rgb_led: &'a mut RgbLed) -> GarageResult<Door<'a>> {
    let open_sensor_receiver = mqtt_channels.open_sensor_receiver();
    let closed_sensor_receiver = mqtt_channels.closed_sensor_receiver();
    let safe_to_close_receiver = mqtt_channels.safe_to_close_receiver();

    log::info!("Getting initial state from sensor");

    rgb_led.on(colors::YELLOW);
    let initial_state = select(
      pin!(async move {
        let open_sensor = open_sensor_receiver.receive().await;
        let closed_sensor = closed_sensor_receiver.receive().await;
        (
          SensorState::from_sensors(open_sensor, closed_sensor),
          open_sensor,
          closed_sensor,
        )
      }),
      pin!(Timer::after(embassy_time::Duration::from_secs(10))),
    )
    .await;
    rgb_led.off();

    let (initial_state, last_open_sensor, last_closed_sensor) = match initial_state {
      Either::First(state) => state,
      Either::Second(_) => return Err(GarageError::DoorInitialisationTimeout),
    };
    log::info!("Initial state: {:?}", initial_state);

    let remote = DoorRemote::new(gpio, rgb_led)?;

    let mut door = Door {
      publisher: mqtt_channels.publisher(),
      command_receiver: mqtt_channels.command_receiver(),
      safe_to_close_receiver,
      open_sensor_receiver,
      closed_sensor_receiver,
      current_state: initial_state.into(),
      last_open_sensor,
      last_closed_sensor,
      safe_to_close: false,
      remote,
    };

    door.publish_current_state().await;

    let initial_target_state =
      TargetState::from_str(&CONFIG.door.initial_target_state).expect("Invalid initial_target_state");

    // If the initial target requires closing, wait for a safe_to_close message first
    if initial_target_state == TargetState::Closed {
      log::info!("Initial target is CLOSED, waiting for safe_to_close status");
      let safe_result = select(
        pin!(async { door.safe_to_close_receiver.receive().await }),
        pin!(Timer::after(embassy_time::Duration::from_secs(10))),
      )
      .await;

      match safe_result {
        Either::First(safe) => {
          door.safe_to_close = safe;
          log::info!("Received initial safe_to_close: {}", safe);
        }
        Either::Second(_) => {
          log::warn!("Timed out waiting for safe_to_close, assuming unsafe");
        }
      }

      if !door.safe_to_close {
        log::warn!("Not safe to close on startup, skipping initial close target");
        door.publish_current_state().await;
      }
      else {
        door.goto_target_state(initial_target_state).await?;
      }
    }
    else {
      door.goto_target_state(initial_target_state).await?;
    }

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
      let action = select4(
        pin!(async {
          let action = select(
            pin!(async { self.open_sensor_receiver.receive().await }),
            pin!(async { self.closed_sensor_receiver.receive().await }),
          )
          .await;

          match action {
            Either::First(open_sensor) => {
              self.last_open_sensor = open_sensor;
            }
            Either::Second(closed_sensor) => {
              self.last_closed_sensor = closed_sensor;
            }
          }

          SensorState::from_sensors(self.last_open_sensor, self.last_closed_sensor)
        }),
        pin!(async {
          // wait for a state expiry to complete (e.g. travel time)
          if let Some(expiry) = self.current_state.expiry_mut() {
            expiry.await;
          }
          else {
            // if there's no expiry don't resolve this branch ever
            future::pending().await
          }
        }),
        pin!(async { self.command_receiver.receive().await }),
        pin!(async { self.safe_to_close_receiver.receive().await }),
      )
      .await;

      // process the action
      match action {
        Either4::First(detected_state) => {
          // detected state changed
          log::info!(
            "Door detected state: {:?}, current state: {:?}",
            &detected_state,
            &self.current_state
          );

          match (&self.current_state, detected_state) {
            (State::Closed, SensorState::Stuck) => {
              self.set_current_state(State::StuckClosed).await
            }
            (State::Open, SensorState::Stuck) => {
              self.set_current_state(State::StuckOpen).await
            }

            (State::Closed | State::AttemptingOpen(_) | State::StuckClosed, SensorState::Moving) => {
              // door was stuck/closed but it's now opening
              log::info!("Door was detected now opening");
              self
                .set_current_state(State::Opening(ConfirmedTravel::new(CONFIG.door.travel_duration)))
                .await
            }
            (State::Open | State::AttemptingClose(_) | State::StuckOpen, SensorState::Moving) => {
              // door was stuck/open but it's now closing
              log::info!("Door was detected now closing");
              self
                .set_current_state(State::Closing(ConfirmedTravel::new(CONFIG.door.travel_duration)))
                .await
            }

            (
              State::Closed | State::AttemptingOpen(_) | State::Opening(_) | State::StuckClosed | State::StuckOpen,
              SensorState::Open,
            ) => {
              // door was closed/stuck/opening and it's now open
              log::info!("Door was opened");
              self.set_current_state(State::Open).await
            }
            (
              State::Open | State::AttemptingClose(_) | State::Closing(_) | State::StuckClosed | State::StuckOpen,
              SensorState::Closed,
            ) => {
              // door was open/stuck/closing and it's now closed
              log::info!("Door was closed");
              self.set_current_state(State::Closed).await
            }

            _ => (), // no-op
          }
        }
        Either4::Second(()) => {
          // expiry resolved
          match &mut self.current_state {
            State::AttemptingOpen(confirmed_travel)
            | State::Opening(confirmed_travel)
            | State::AttemptingClose(confirmed_travel)
            | State::Closing(confirmed_travel) => {
              // the door didn't open/close as it was requested to
              if confirmed_travel.reattempt().is_ok() {
                // the travel expired, i.e. the door didn't move in to place before it should have
                // travel is still the current state at this point, so we can safely assume it hasn't completed

                // we're going to try again
                log::info!("Door failed to move, triggering remote again");
                self.remote.trigger().await?;
              }
              else {
                // we've tried too many times
                log::info!("Door failed to move after maximum attemps, marking as stuck");
                match self.current_state {
                  // Attempting to open but failed => stuck closed
                  State::AttemptingOpen(_) | State::Opening(_) => self.set_current_state(State::StuckClosed).await,
                  // Attempting to close but failed => stuck open
                  State::AttemptingClose(_) | State::Closing(_) => self.set_current_state(State::StuckOpen).await,
                  _ => unreachable!(),
                }
              }
            }
            State::Open | State::StuckOpen | State::Closed | State::StuckClosed => {
              unreachable!("state should not have an expiry")
            }
          }
        }
        Either4::Third(target_state) => {
          // command received
          if target_state == TargetState::Closed && !self.safe_to_close {
            log::warn!("Ignoring close command: not safe to close");
          }
          else {
            log::info!("Next target state: {:?}", target_state);
            next_target_state = Some(target_state);
          }
        }
        Either4::Fourth(safe_to_close) => {
          log::info!("Safe to close updated: {}", safe_to_close);
          self.safe_to_close = safe_to_close;
        }
      }
    }
  }

  async fn set_current_state(&mut self, current_state: State) {
    log::info!("Door setting new state: {:?}", current_state);
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
          // we can detect if the door starts to close, so ensure it does
          self
            .set_current_state(State::AttemptingClose(ConfirmedTravel::new(
              CONFIG.door.remote.max_latency_duration
                + CONFIG.door.remote.pressed_duration
                + CONFIG.door.remote.wait_duration,
            )))
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
      log::info!("Door is now targeting state {}, triggering remote", target_state);
      self.remote.trigger().await?;
    }

    Ok(())
  }
}
