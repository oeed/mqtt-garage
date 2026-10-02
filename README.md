# mqtt-garage

An ESP32-S3 firmware written in Rust that controls a garage door via MQTT. It reads two wired reed switches marking the ends of the door's travel, receives open/close commands over MQTT, and drives the opener's push-button and safety inputs through relays.

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

## Wiring

All low-voltage wiring shares one ground: opener `GND` = LM2596 buck `GND` = ESP32 `GND`.

| ESP32-S3 | Goes to | Notes |
|---|---|---|
| `GPIO14` | Open relay `IN` | Relay `COM`/`NO` across opener `PB`↔`GND` (momentary pulse = button press) |
| `GPIO13` | Safety relay `IN` | Relay `COM`/`NC` across opener `PE`↔`GND` (energised = closing inhibited) |
| `GPIO4` | Open (top) reed switch | Switch's other terminal to `GND` |
| `GPIO5` | Closed (bottom) reed switch | Switch's other terminal to `GND` |

The reed switches (e.g. Jaycar LA5072) close when their magnet is alongside, pulling the pin low; the
firmware enables the internal pull-up, but over a long run past the motor fit this at the ESP end of
each sensor cable:

```
3V3 ──[4.7kΩ]──┬──[1kΩ]──┬── GPIO4 / GPIO5
               │         │
     reed switch      [100nF]
               │         │
GND ───────────┴─────────┘
```

The 4.7kΩ pull-up gives the line a stiff idle level, and the 1kΩ + 100nF filter knocks down motor
noise and protects the pin. Use twisted pair (e.g. one pair of a Cat5 offcut) for each sensor. Never
connect a sensor to `5V` or the opener's `24V` — ESP32-S3 inputs are 3.3V only. A cut wire reads as
"no contact", so it can make the door look unverified but never falsely closed.

Mount the closed sensor so it only makes contact with the door fully down, and the open sensor so it
only makes contact with the door fully up (within the switch's ~15–20mm gap).

## Architecture

- **Target:** ESP32-S3 (`xtensa-esp32s3-espidf`)
- **Async runtime:** Embassy (`embassy-executor`, `embassy-time`, `embassy-sync`)
- **ESP-IDF bindings:** `esp-idf-svc` v0.51
- **MQTT topics:**
  - Subscribes to a command topic for open/close commands
  - Subscribes to a safe-to-close topic — close commands are ignored when this is `"false"`
  - Publishes door state, stuck status, each reed switch's contact (`{"contact":true|false}`,
    retained), and availability
- **Door state machine:** a commanded travel is budgeted to last until the door should have reached
  the *far* sensor, so the door is never interrupted part-way through its traverse. Arrival at either
  sensor proves the position on its own. When a travel runs out of time, the live sensor reading says
  where the door ended up (still at its origin, or at neither sensor). Only a sensor clears a target: a
  travel that ends unconfirmed spends another pulse (up to `max_attempts`). Pulses land on a
  stationary door and therefore reverse it, so
  `max_attempts` must be **odd** for the door to finish at the commanded end when nothing ever
  confirms. Once the attempts run out the position is reported as open and stuck, since a door that
  might be open must not be mistaken for a secured one.
- **Build-time config:** TOML config is parsed in `build.rs` and embedded as a static `Config` struct
- **Watchdog:** task watchdog (8s), interrupt watchdog (300ms), and an async stall detector that reboots if the executor stalls for >20s
- **Core dumps:** saved to flash in ELF format for post-mortem debugging
