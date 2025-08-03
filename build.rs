use std::{env, fs, path::PathBuf};

use config::Config;

fn main() {
  embuild::espidf::sysenv::output();

  const CONFIG_FILE: &str = "garage-config.toml";

  println!("cargo:rerun-if-changed={}", CONFIG_FILE);

  let config_path = PathBuf::from(CONFIG_FILE);
  let config_str = fs::read_to_string(config_path).unwrap_or_else(|e| panic!("Failed to read {}: {}", CONFIG_FILE, e));

  let config: Config = toml::from_str(&config_str).unwrap_or_else(|e| panic!("Failed to parse {}: {}", CONFIG_FILE, e));

  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
  let dest_path = out_dir.join("config_generated.rs");

  let generated_code = format!(
    r#"
      use std::borrow::Cow;
      pub static CONFIG: Config = Config {{
        wifi: WifiConfig {{
          ssid: Cow::Borrowed("{wifi_ssid}"),
          password: Cow::Borrowed("{wifi_psk}"),
        }},
        mqtt: MqttConfig {{
          url: Cow::Borrowed("{mqtt_url}"),
          client_id: Cow::Borrowed("{mqtt_client_id}"),
          availability_topic: Cow::Borrowed("{mqtt_availability_topic}"),
          online_availability: Cow::Borrowed("{mqtt_online_payload}"),
          offline_availability: Cow::Borrowed("{mqtt_offline_payload}"),
        }},
        door: DoorConfig {{
          command_topic: Cow::Borrowed("{door_cmd_topic}"),
          initial_target_state: Cow::Borrowed("{door_initial_target_state}"),
          state_topic: Cow::Borrowed("{door_state_topic}"),
          stuck_topic: Cow::Borrowed("{door_stuck_topic}"),
          travel_duration: embassy_time::Duration::from_millis({door_travel_duration_ms}),
          remote: RemoteConfig {{
              pin: Cow::Borrowed("{door_remote_pin}"),
              pressed_duration: embassy_time::Duration::from_millis({door_remote_pressed_time_ms}),
              wait_duration: embassy_time::Duration::from_millis({door_remote_wait_time_ms}),
              max_latency_duration: embassy_time::Duration::from_millis({door_remote_max_latency_duration_ms}),
          }},
          sensor_topic: Cow::Borrowed("{door_detector_sensor_topic}"),
          max_attempts: {door_max_attempts},
        }},
      }};
  "#,
    // WIFI
    wifi_ssid = config.wifi.ssid,
    wifi_psk = config.wifi.password,
    // MQTT
    mqtt_url = config.mqtt.url,
    mqtt_client_id = config.mqtt.client_id,
    mqtt_availability_topic = config.mqtt.availability_topic,
    mqtt_online_payload = config.mqtt.online_availability,
    mqtt_offline_payload = config.mqtt.offline_availability,
    // Door Controller
    door_cmd_topic = config.door.command_topic,
    door_state_topic = config.door.state_topic,
    door_stuck_topic = config.door.stuck_topic,
    door_travel_duration_ms = config.door.travel_duration.as_millis(),
    door_initial_target_state = config.door.initial_target_state,
    // Door Remote
    door_remote_pin = config.door.remote.pin,
    door_remote_pressed_time_ms = config.door.remote.pressed_duration.as_millis(),
    door_remote_wait_time_ms = config.door.remote.wait_duration.as_millis(),
    door_remote_max_latency_duration_ms = config.door.remote.max_latency_duration.as_millis(),
    // Door Detector
    door_detector_sensor_topic = config.door.sensor_topic,
    door_max_attempts = config.door.max_attempts,
  );

  fs::write(&dest_path, generated_code).unwrap();
}
