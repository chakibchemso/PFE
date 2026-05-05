//! Secret configuration template.
//! Copy this file to `config.rs` and fill in your actual credentials.
//! `config.rs` is gitignored — keep it out of version control.

// WiFi
pub const WIFI_SSID: &str = "your-ssid-here";
pub const WIFI_PASSWORD: &str = "your-password-here";

// MQTT
pub const MQTT_HOST: &str = "mqtt.flespi.io";
pub const MQTT_PORT: u16 = 1883;
pub const MQTT_USERNAME: &str = "your-token-here";
pub const MQTT_CLIENT_ID: &str = "your-client-id";
pub const MQTT_TOPIC: &str = "your-namespace/esp32/data";

// Crypto
pub const ASCON_KEY: &[u8; 16] = b"CHANGE_ME_KEY!";
