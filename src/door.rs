use std::{future, pin::pin, str::FromStr};

use embassy_futures::select::{Either, Either4, select, select4};
use esp_idf_svc::{
  hal::gpio::{Gpio4, Gpio5, Gpio13, Gpio14},
  mqtt::client::QoS,
};

use self::{
  remote::DoorRemote,
  safety::SafetyRelay,
  sensors::DoorSensors,
  state::{DoorCommand, SensorState, State, TargetState, Travel},
};
use crate::{
  config::CONFIG,
  error::GarageResult,
  mqtt_client::{MqttChannels, MqttPublish, MqttTopicPublisher, MqttTopicReceiver},
  rgb::RgbLed,
};

pub mod remote;
pub mod safety;
pub mod sensors;
pub mod state;

const CONTACT_PAYLOAD: &str = r#"{"contact":true}"#;
const NO_CONTACT_PAYLOAD: &str = r#"{"contact":false}"#;

/// A position the door has been asked to reach, held onto until a sensor confirms it is there or the
/// attempts run out.
#[derive(Debug, Clone, Copy)]
struct DoorTarget {
  state: TargetState,
  /// Relay pulses spent on this target so far.
  attempts: u8,
}

/// Whatever the door loop woke up for.
enum Action {
  /// The contact sensors settled on a new reading.
  Sensor(SensorState),
  /// A travel ran its full course without the door arriving at the far sensor.
  TravelExpired,
  Command(DoorCommand),
  SafeToClose(bool),
}

pub struct Door<'a> {
  publisher: MqttTopicPublisher<'a>,
  command_receiver: MqttTopicReceiver<'a, DoorCommand>,
  safe_to_close_receiver: MqttTopicReceiver<'a, bool>,

  /// Wired reed switches at the top and bottom of the travel.
  sensors: DoorSensors,
  /// Whether it is currently safe to close the door. Starts `false` until told otherwise.
  safe_to_close: bool,
  /// Where the door has been asked to be, if it isn't confirmed to be there yet.
  target: Option<DoorTarget>,

  remote: DoorRemote<'a>,
  /// Relay across the opener's safety input (PE ↔ GND); tracks `safe_to_close`.
  safety_relay: SafetyRelay,
  current_state: State,
}

impl<'a> Door<'a> {
  pub async fn new(
    remote_gpio: Gpio14,
    safety_gpio: Gpio13,
    open_sensor_gpio: Gpio4,
    closed_sensor_gpio: Gpio5,
    mqtt_channels: &'a MqttChannels,
    rgb_led: &'a mut RgbLed,
  ) -> GarageResult<Door<'a>> {
    let sensors = DoorSensors::new(open_sensor_gpio, closed_sensor_gpio).await?;
    let initial_state = sensors.state();
    log::info!("Initial sensor state: {:?}", initial_state);

    let remote = DoorRemote::new(remote_gpio, rgb_led)?;
    let safety_relay = SafetyRelay::new(safety_gpio)?;

    let mut door = Door {
      publisher: mqtt_channels.publisher(),
      command_receiver: mqtt_channels.command_receiver(),
      safe_to_close_receiver: mqtt_channels.safe_to_close_receiver(),
      current_state: initial_state.into(),
      sensors,
      safe_to_close: false,
      target: None,
      remote,
      safety_relay,
    };

    // Reflect the initial (conservative) safe-to-close belief onto the physical interlock relay.
    let initial_safe = door.safe_to_close;
    door.set_safe_to_close(initial_safe)?;

    door.publish_current_state().await;
    door.publish_sensor_contacts(initial_state).await;

    let initial_target_state =
      TargetState::from_str(&CONFIG.door.initial_target_state).expect("Invalid initial_target_state");

    // If the initial target requires closing, wait for a safe_to_close message first
    if initial_target_state == TargetState::Closed {
      log::info!("Initial target is CLOSED, waiting for safe_to_close status");
      let safe_result = select(
        pin!(async { door.safe_to_close_receiver.receive().await }),
        pin!(embassy_time::Timer::after(embassy_time::Duration::from_secs(10))),
      )
      .await;

      match safe_result {
        Either::First(safe) => {
          door.set_safe_to_close(safe)?;
          log::info!("Received initial safe_to_close: {}", safe);
        }
        Either::Second(_) => {
          log::warn!("Timed out waiting for safe_to_close, assuming unsafe");
        }
      }
    }

    // Hand the target to the loop rather than acting on it here, so startup goes through exactly the
    // same drive-and-confirm path as any other command — including the safe-to-close gate.
    door.target = Some(DoorTarget {
      state: initial_target_state,
      attempts: 0,
    });

    Ok(door)
  }

  pub async fn listen(mut self) -> GarageResult<()> {
    log::info!("Door listening with initial state: {:?}", self.current_state);
    loop {
      // Drive the standing target. Only ever acts on a stationary door, so a travel already underway
      // is never interrupted.
      if !self.current_state.is_travelling() {
        self.drive_target().await?;
      }

      // determine what action is ready to be processed
      let action = select4(
        pin!(async { Action::Sensor(self.sensors.next_change().await) }),
        pin!(async {
          // wait for the in-flight travel to run out of time
          if let Some(expiry) = self.current_state.expiry_mut() {
            expiry.await;
          }
          else {
            // if there's no travel don't resolve this branch ever
            future::pending().await
          }
          Action::TravelExpired
        }),
        pin!(async { Action::Command(self.command_receiver.receive().await) }),
        pin!(async { Action::SafeToClose(self.safe_to_close_receiver.receive().await) }),
      )
      .await;

      let action = match action {
        Either4::First(action) | Either4::Second(action) | Either4::Third(action) | Either4::Fourth(action) => action,
      };

      // process the action
      match action {
        Action::Sensor(detected_state) => {
          self.publish_sensor_contacts(detected_state).await;
          self.process_detected_state(detected_state).await;
        }
        Action::TravelExpired => {
          // The door had the whole traverse to reach its destination sensor and didn't. The sensors
          // are read live, so they say what happened: still at the origin sensor means it never moved
          // (or went straight back), at neither means it stopped part-way — and unknown reads as open,
          // since a door that might be open must never be mistaken for a secured one. If a target is
          // still standing, the loop will spend another pulse on it.
          let settled = State::from(self.sensors.state());
          log::warn!(
            "Travel ({:?}) ran out of time without reaching the far sensor; door is now {:?}",
            self.current_state,
            settled
          );
          self.set_current_state(settled).await;
        }
        Action::Command(command) => {
          match command {
            DoorCommand::Target(target_state) => {
              if target_state == TargetState::Closed && !self.safe_to_close {
                log::warn!("Ignoring close command: not safe to close");
              }
              else {
                log::info!("Next target state: {:?}", target_state);
                // A fresh command supersedes whatever was being attempted, with a fresh set of attempts.
                self.target = Some(DoorTarget {
                  state: target_state,
                  attempts: 0,
                });
              }
            }
            DoorCommand::Trigger => {
              // Debug/testing: pulse the relay directly, just like a handheld remote. This deliberately
              // ignores the safe-to-close gate and does not set a target state — whatever the door
              // physically does is then picked up by the contact sensors and reflected in the state.
              // Any standing target is dropped so the retry logic doesn't fight the manual press.
              log::warn!("Pulsing remote directly via trigger command (bypassing safe-to-close)");
              self.target = None;
              self.remote.trigger().await?;
            }
          }
        }
        Action::SafeToClose(safe_to_close) => {
          log::info!("Safe to close updated: {}", safe_to_close);
          self.set_safe_to_close(safe_to_close)?;
        }
      }
    }
  }

  /// Apply a debounced sensor state change to the current door state.
  async fn process_detected_state(&mut self, detected_state: SensorState) {
    log::info!(
      "Door detected state: {:?}, current state: {:?}",
      &detected_state,
      &self.current_state
    );

    match (&self.current_state, detected_state) {
      // Already where the sensors say the door is.
      (State::Open, SensorState::Open) | (State::Closed, SensorState::Closed) => (),

      // A sensor is in contact and has been for the whole debounce window. Contact is positive
      // evidence — a reed switch has to see a magnet that isn't there to report it falsely, whereas
      // failing to report needs only a broken wire or a knocked magnet — so it settles the position
      // from any state, including a door that reverses part-way back to where it started.
      (_, SensorState::Open) => {
        log::info!("Door is open");
        self.set_current_state(State::Open).await
      }
      (_, SensorState::Closed) => {
        log::info!("Door is closed");
        self.set_current_state(State::Closed).await
      }

      // Movement while the door was at rest: someone else is operating it — a handheld remote, the
      // wall button, or the door being pushed. The relay is never actuated on this path.
      (State::Closed | State::StuckClosed, SensorState::Moving) => {
        log::info!("Door was detected now opening");
        self.set_current_state(State::Opening(Travel::observed())).await
      }
      (State::Open | State::StuckOpen, SensorState::Moving) => {
        log::info!("Door was detected now closing");
        self.set_current_state(State::Closing(Travel::observed())).await
      }
      // Already travelling: this is just the door somewhere between its two sensors, which is what a
      // travel already means. It must not restart the deadline, or a door that keeps reporting
      // movement would never reach a verdict.
      (State::Opening(_) | State::Closing(_), SensorState::Moving) => (),

      // Two sensors contradicting each other. One of them is faulty, but not which — so the believed
      // position stands and simply stops counting as verified. The per-sensor contact topics show
      // which switch is misreporting.
      (State::Closed, SensorState::Conflict) => self.set_current_state(State::StuckClosed).await,
      (State::Open, SensorState::Conflict) => self.set_current_state(State::StuckOpen).await,
      (State::StuckClosed | State::StuckOpen | State::Opening(_) | State::Closing(_), SensorState::Conflict) => (),
    }
  }

  async fn set_current_state(&mut self, current_state: State) {
    log::info!("Door setting new state: {:?}", current_state);
    self.current_state = current_state;
    self.publish_current_state().await
  }

  /// Update the cached safe-to-close status and drive the physical interlock relay to match: its
  /// NC contact across the opener's safety input is de-energized (closed → closing permitted) when
  /// safe, and energized (open → closing inhibited) when not.
  fn set_safe_to_close(&mut self, safe_to_close: bool) -> GarageResult<()> {
    self.safe_to_close = safe_to_close;
    self.safety_relay.set_safe_to_close(safe_to_close)
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

  /// Publish each sensor's contact on its own topic, so a switch that has failed (a cut wire never
  /// reports contact; a short always does) can be spotted directly rather than inferred from the door
  /// misbehaving.
  async fn publish_sensor_contacts(&self, sensor_state: SensorState) {
    let payload = |contact: bool| if contact { CONTACT_PAYLOAD } else { NO_CONTACT_PAYLOAD };

    self
      .publisher
      .publish(MqttPublish {
        topic: &CONFIG.door.open_sensor_state_topic,
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: payload(sensor_state.open_contact()),
      })
      .await;

    self
      .publisher
      .publish(MqttPublish {
        topic: &CONFIG.door.closed_sensor_state_topic,
        qos: QoS::AtLeastOnce,
        retain: true,
        payload: payload(sensor_state.closed_contact()),
      })
      .await;
  }

  /// Spend a pulse on the standing target, if there is one and it is neither reached nor exhausted.
  ///
  /// Only a sensor clears the target. Another pulse covers the opener ignoring a press, the door
  /// reversing off an obstruction, or the door stopping part-way.
  ///
  /// Pulses land on a stationary door and so reverse it. A door at a known end is only ever pulsed
  /// towards the target, but one stopped at neither sensor (or one whose destination sensor has failed)
  /// may go either way — which is why `max_attempts` wants to be odd, so a door that never confirms
  /// anything still finishes at the commanded end.
  async fn drive_target(&mut self) -> GarageResult<()> {
    let Some(target) = self.target
    else {
      return Ok(());
    };

    if self.current_state == target.state {
      log::info!("Door confirmed {} after {} attempt(s)", target.state, target.attempts);
      self.target = None;
    }
    else if target.attempts >= CONFIG.door.max_attempts {
      log::warn!(
        "Door never confirmed {} after {} attempts; leaving it as {:?}",
        target.state,
        target.attempts,
        self.current_state
      );
      self.target = None;
    }
    else if self.pulse_may_close() && !self.safe_to_close {
      // The next pulse would send a door we believe to be up back down, which the interlock physically
      // prevents — so the sequence cannot get any further.
      log::warn!("Not safe to close; abandoning attempts to reach {}", target.state);
      self.target = None;
    }
    else {
      self.target = Some(DoorTarget {
        attempts: target.attempts + 1,
        ..target
      });

      // The travel is budgeted to last until the door should have reached the far sensor, so a door
      // part-way through its traverse is never interrupted by its own controller. Its direction is a
      // belief, not a command — a single-button opener only toggles — so if the door turns out to be
      // going the other way, the sensors correct it.
      match target.state {
        TargetState::Closed => self.set_current_state(State::Closing(Travel::commanded())).await,
        TargetState::Open => self.set_current_state(State::Opening(Travel::commanded())).await,
      }
      log::info!(
        "Door is now targeting state {} (attempt {} of {}), triggering remote",
        target.state,
        target.attempts + 1,
        CONFIG.door.max_attempts
      );
      self.remote.trigger().await?;
    }

    Ok(())
  }

  /// Whether the next pulse would set the door closing, going on where the door is believed to be.
  fn pulse_may_close(&self) -> bool {
    matches!(self.current_state, State::Open | State::StuckOpen)
  }
}
