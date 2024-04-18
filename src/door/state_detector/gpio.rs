use std::{thread::sleep, time::Duration};

use async_trait::async_trait;
#[cfg(feature = "arm")]
use rppal::gpio::{Gpio, InputPin};
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};

use super::{DetectedState, StateDetector, Travel};
#[cfg(not(feature = "arm"))]
use crate::mock_gpio::{Gpio, InputPin};
use crate::{
  config::gpio::GpioPin,
  door::{
    state::{State, TargetState},
    Identifier,
  },
  error::GarageResult,
};

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct GpioStateDetectorConfig {
  /// The pin of the door detector sensor (if available)
  pub pin: GpioPin,

  #[serde_as(as = "DurationSeconds<u64>")]
  /// How long the door is expected to take to go to/from open/close.
  ///
  /// If it exceeds this it tries again.
  pub travel_time: Duration,
}


#[derive(Debug)]
pub struct GpioStateDetector {
  pin: InputPin,
  travel_time: Duration,
  current_travel: Option<Travel>,
}

impl GpioStateDetector {
  /// Take a single reading of the pin
  fn pin_state(&self) -> DetectedState {
    if self.pin.is_high() {
      DetectedState::Open
    }
    else {
      DetectedState::Closed
    }
  }

  /// Take multiple readings until we get stable state
  fn stable_state(&self) -> DetectedState {
    const MAX_READS: usize = 50;
    const MIN_CONSECUTIVE: usize = 10;

    let mut previous_state = None;
    let mut consecutive = 0;
    for _ in 0..MAX_READS {
      if let Some(prev_state) = previous_state {
        let state = self.pin_state();
        if state == prev_state {
          consecutive += 1;
          if consecutive >= MIN_CONSECUTIVE {
            return state;
          }
        }
        else {
          consecutive = 0;
          previous_state = Some(state);
        }
      }
      else {
        previous_state = Some(self.pin_state())
      }

      sleep(Duration::from_millis(20))
    }

    // we didn't get enough consecutive readings, we're possibly stuck
    DetectedState::Stuck
  }
}

#[async_trait]
impl StateDetector for GpioStateDetector {
  type Config = GpioStateDetectorConfig;

  fn with_config(_: Identifier, config: Self::Config) -> GarageResult<Self> {
    let gpio = Gpio::new()?;
    let pin = gpio.get(config.pin.bcm_number())?.into_input_pullup();

    Ok(GpioStateDetector {
      pin,
      travel_time: config.travel_time,
      current_travel: None,
    })
  }

  async fn travel(&mut self, target_state: TargetState) -> DetectedState {
    if self.current_travel.is_some() {
      panic!("SensorStateDetector attempted to travel while it was already travelling");
    }
    self.current_travel = Some(Travel::new(target_state));
    tokio::time::sleep(self.travel_time).await;
    self.detect_state()
  }

  fn detect_state(&mut self) -> DetectedState {
    let detected_state: DetectedState = self.stable_state();

    // check if this state indicates the door might be stuck
    if let Some(current_travel) = self.current_travel.take() {
      if current_travel.expired_invalid(detected_state, self.travel_time) {
        return DetectedState::Stuck;
      }
    }

    detected_state
  }

  fn should_check(&self) -> bool {
    true
  }

  fn manual_travel_state(&self, target_state: TargetState) -> State {
    match target_state {
      // the sensor indicates if it's closed, so once no longer closed we assume opening
      TargetState::Open => State::Opening,
      TargetState::Closed => State::Closed,
    }
  }
}
