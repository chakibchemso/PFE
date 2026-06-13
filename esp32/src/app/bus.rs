//! System Bus — central IPC manifest for the smartwatch OS.
//!
//! All inter-service channels are defined here. Services extract only the
//! senders/receivers they need; no service imports another service's internals.

use alloc::vec::Vec;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;

/// Battery state published by the power service.
#[derive(Clone, Copy, defmt::Format)]
pub struct BatteryState {
    pub pct: Option<u8>,
    pub charging: bool,
}

/// Parsed GPS fix data published by the GPS service.
#[derive(Clone, Copy, Default, defmt::Format)]
pub struct GpsFix {
    pub lat: f32,
    pub lon: f32,
    pub speed_kmh: f32,
    pub heading_deg: f32,
    pub altitude_m: f32,
    pub satellites: u8,
    pub has_fix: bool,
    pub fix_quality: u8,
}

impl GpsFix {
    pub const fn no_fix() -> Self {
        Self {
            lat: 0.0,
            lon: 0.0,
            speed_kmh: 0.0,
            heading_deg: 0.0,
            altitude_m: 0.0,
            satellites: 0,
            has_fix: false,
            fix_quality: 0,
        }
    }
}

pub struct SystemBus {
    /// Current vitals: (bpm: u8, spo2: u8, temp: u8)
    pub vitals: Watch<CriticalSectionRawMutex, (u8, u8, u8), 2>,
    /// Current ECG heart rate (BPM)
    pub ecg_hr: Watch<CriticalSectionRawMutex, (u8,), 2>,
    /// WiFi connectivity state
    pub wifi_status: Watch<CriticalSectionRawMutex, bool, 2>,
    /// MQTT broker connectivity state
    pub mqtt_status: Watch<CriticalSectionRawMutex, bool, 2>,
    /// GPS fix data
    pub gps: Watch<CriticalSectionRawMutex, Option<GpsFix>, 2>,
    /// Encrypted payload queue (sensing → MQTT)
    pub data_channel: Channel<CriticalSectionRawMutex, Vec<u8>, 5>,
    /// ESP32 die temperature in Celsius
    pub cpu_temp: Watch<CriticalSectionRawMutex, i8, 2>,
    /// UTC epoch seconds, updated by the NTP sync service
    pub utc_epoch: Watch<CriticalSectionRawMutex, u64, 2>,
    /// Battery state: (percentage, is_charging)
    pub battery: Watch<CriticalSectionRawMutex, BatteryState, 2>,
}

impl SystemBus {
    pub const fn new() -> Self {
        Self {
            vitals: Watch::new(),
            ecg_hr: Watch::new(),
            wifi_status: Watch::new(),
            mqtt_status: Watch::new(),
            gps: Watch::new(),
            data_channel: Channel::new(),
            cpu_temp: Watch::new(),
            utc_epoch: Watch::new(),
            battery: Watch::new(),
        }
    }
}
