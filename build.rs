use std::{env, fs, path::PathBuf};

use config::Config;
use serde::Deserialize;

fn main() {
  const CONFIG_FILE: &str = "garage-config.toml";

  println!("cargo:rerun-if-changed={}", CONFIG_FILE);

  let config_path = PathBuf::from(CONFIG_FILE);
  let config_str = fs::read_to_string(config_path).unwrap_or_else(|e| panic!("Failed to read {}: {}", CONFIG_FILE, e));

  let config: Config = toml::from_str(&config_str).unwrap_or_else(|e| panic!("Failed to parse {}: {}", CONFIG_FILE, e));

  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
  let dest_path = out_dir.join("config_generated.rs");

  let generated_code = format!(
    r#"
pub static CONFIG: Config = Config {{
    wifi: WifiConfig {{
        ssid: "{wifi_ssid}",
        password: "{wifi_psk}",
    }},
    mqtt: MqttConfig {{
        broker_domain: "{mqtt_host}",
        broker_port: {mqtt_port},
        client_id: "{mqtt_client_id}",
        availability_topic: "{mqtt_availability_topic}",
        online_availability: "{mqtt_online_payload}",
        offline_availability: "{mqtt_offline_payload}",
    }},
    door: DoorConfig {{
        controller: ControllerConfig {{
            command_topic: "{door_cmd_topic}",
            initial_target_state: "{door_initial_target_state}",
            state_topic: "{door_state_topic}",
            stuck_topic: "{door_stuck_topic}",
            travel_duration: {door_travel_duration_secs},
            remote: RemoteConfig {{
                pin: "{door_remote_pin}",
                pressed_time: {door_remote_pressed_time_s},
                wait_time: {door_remote_wait_time_s},
            }},
        }},
        detector: DetectorConfig {{
            sensor_topic: "{door_detector_sensor_topic}",
        }},
    }},
}};
"#,
    // WIFI
    wifi_ssid = config.wifi.ssid,
    wifi_psk = config.wifi.password,
    // MQTT
    mqtt_host = config.mqtt.broker_domain,
    mqtt_port = config.mqtt.broker_port,
    mqtt_client_id = config.mqtt.client_id,
    mqtt_availability_topic = config.mqtt.availability_topic,
    mqtt_online_payload = config.mqtt.online_availability,
    mqtt_offline_payload = config.mqtt.offline_availability,
    // Door Controller
    door_cmd_topic = config.door.controller.command_topic,
    door_state_topic = config.door.controller.state_topic,
    door_stuck_topic = config.door.controller.stuck_topic,
    door_travel_duration_secs = config.door.controller.travel_duration,
    door_initial_target_state = config.door.controller.initial_target_state,
    // Door Remote
    door_remote_pin = config.door.controller.remote.pin,
    door_remote_pressed_time_s = config.door.controller.remote.pressed_time,
    door_remote_wait_time_s = config.door.controller.remote.wait_time,
    // Door Detector
    door_detector_sensor_topic = config.door.detector.sensor_topic
  );

  fs::write(&dest_path, generated_code).unwrap();
}
