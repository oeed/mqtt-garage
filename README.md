# mqtt-garage

An ESP32-S3 firmware written in Rust that controls a garage door via MQTT. It listens to Zigbee door sensors (via zigbee2mqtt), receives open/close commands over MQTT, and triggers a garage door remote via GPIO.

## Prerequisites

- **Espressif Rust toolchain** — the project uses `channel = "esp"` (see `rust-toolchain.toml`). Install via [espup](https://github.com/esp-rs/espup):
  ```bash
  cargo install espup
  espup install
  ```
- **espflash** — used for flashing and monitoring:
  ```bash
  cargo install espflash
  ```
- **ESP-IDF v5.2.3** — automatically downloaded and managed by `embuild` during the first build.
- **macOS serial driver** — if using a CH340X-based USB-serial adapter, install the [WCH CH34X driver](https://github.com/WCHSoftGroup/ch34xser_macos) (install the DMG, then enable the extension in System Settings > Privacy & Security).

## Configuration

Configuration is baked in at compile time via `build.rs`. There are two TOML config files:

| File | Used when |
|---|---|
| `garage-config.debug.toml` | `cargo build` (debug) |
| `garage-config.release.toml` | `cargo build --release` |

Both are gitignored. Copy one of them from a teammate or create your own. The config includes WiFi credentials, MQTT broker details, and door-specific settings (sensor topics, command topics, timing, etc.).

## Building

```bash
# Debug build
cargo build

# Release build (optimised, opt-level 3)
cargo build --release
```

The first build will download and compile ESP-IDF, which can take a while.

## Flashing & Monitoring

The Cargo runner is pre-configured in `.cargo/config.toml` to flash and open a serial monitor in one step:

```bash
# Build, flash, and monitor (debug)
cargo run

# Build, flash, and monitor (release)
cargo run --release
```

This runs `espflash flash --monitor --partition-table partitions.csv` under the hood.

### Manual flash

```bash
espflash flash --monitor --partition-table partitions.csv target/xtensa-esp32s3-espidf/release/mqtt-garage
```

### Serial port

The serial port is configured in `espflash_ports.toml`. Update it to match your device:

```toml
[connection]
serial = "/dev/cu.usbmodem1101"
```

### Monitor only (without flashing)

```bash
# Using the included script
./monitor.sh

# Or manually (exit with Ctrl+A then Ctrl+K)
screen /dev/tty.wchusbserial58FA0381661 115200
```

## Simulation (Wokwi)

The project includes a [Wokwi](https://wokwi.com/) configuration for local simulation and debugging without physical hardware:

```bash
# Build debug, then run in Wokwi via the VS Code extension
cargo build
```

A VS Code launch configuration (`.vscode/launch.json`) is provided for GDB debugging through Wokwi.

## Architecture

- **Target:** ESP32-S3 (`xtensa-esp32s3-espidf`)
- **Async runtime:** Embassy (`embassy-executor`, `embassy-time`, `embassy-sync`)
- **ESP-IDF bindings:** `esp-idf-svc` v0.51
- **MQTT topics:**
  - Subscribes to Zigbee door sensor topics (open + closed contact sensors)
  - Subscribes to a command topic for open/close commands
  - Subscribes to a safe-to-close topic — close commands are ignored when this is `"false"`
  - Publishes door state, stuck status, and availability
- **Build-time config:** TOML config is parsed in `build.rs` and embedded as a static `Config` struct
- **Watchdog:** task watchdog (8s), interrupt watchdog (300ms), and an async stall detector that reboots if the executor stalls for >20s
- **Core dumps:** saved to flash in ELF format for post-mortem debugging
