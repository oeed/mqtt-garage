//! Home Assistant MQTT discovery for the entities this device can describe itself.
//!
//! Each config is published retained, so Home Assistant picks the entities up whenever it starts, and
//! they are tied to a single device so they show up together under Settings → Devices.

use std::sync::LazyLock;

use crate::config::CONFIG;

/// A discovery config message: the topic Home Assistant watches, and the entity definition to send it.
pub struct DiscoveryConfig {
  pub topic: String,
  pub payload: String,
}

/// The device every discovered entity is grouped under.
fn device_json() -> String {
  format!(
    r#"{{"identifiers":["{client_id}"],"name":"Garage Door Controller","manufacturer":"oeed","model":"mqtt-garage","sw_version":"{version}"}}"#,
    client_id = CONFIG.mqtt.client_id,
    version = env!("CARGO_PKG_VERSION"),
  )
}

/// A binary sensor that is on while the given reed switch is in contact with its magnet — i.e. while
/// the door is at that end of its travel.
fn contact_sensor(key: &str, name: &str, icon: &str, state_topic: &str) -> DiscoveryConfig {
  let client_id = &CONFIG.mqtt.client_id;
  DiscoveryConfig {
    topic: format!(
      "{prefix}/binary_sensor/{client_id}/{key}/config",
      prefix = CONFIG.mqtt.discovery_prefix
    ),
    payload: format!(
      r#"{{"name":"{name}","unique_id":"{client_id}_{key}","default_entity_id":"binary_sensor.garage_{key}","icon":"{icon}","state_topic":"{state_topic}","value_template":"{{{{ 'ON' if value_json.contact else 'OFF' }}}}","availability_topic":"{availability_topic}","payload_available":"{online}","payload_not_available":"{offline}","device":{device}}}"#,
      availability_topic = CONFIG.mqtt.availability_topic,
      online = CONFIG.mqtt.online_availability,
      offline = CONFIG.mqtt.offline_availability,
      device = device_json(),
    ),
  }
}

/// Every discovery config this device publishes.
pub static DISCOVERY_CONFIGS: LazyLock<[DiscoveryConfig; 2]> = LazyLock::new(|| {
  [
    contact_sensor(
      "open_sensor",
      "Open Sensor",
      "mdi:arrow-collapse-up",
      &CONFIG.door.open_sensor_state_topic,
    ),
    contact_sensor(
      "closed_sensor",
      "Closed Sensor",
      "mdi:arrow-collapse-down",
      &CONFIG.door.closed_sensor_state_topic,
    ),
  ]
});
