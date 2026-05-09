//! System Bus — central IPC manifest for the smartwatch OS.
//!
//! All inter-service channels are defined here. Services extract only the
//! senders/receivers they need; no service imports another service's internals.

use alloc::vec::Vec;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;

pub struct SystemBus {
    /// Current vitals: (bpm: u8, spo2: u8, temp: u8)
    pub vitals: Watch<CriticalSectionRawMutex, (u8, u8, u8), 2>,
    /// WiFi connectivity state
    pub wifi_status: Watch<CriticalSectionRawMutex, bool, 2>,
    /// Encrypted payload queue (sensing → MQTT)
    pub data_channel: Channel<CriticalSectionRawMutex, Vec<u8>, 5>,
}

impl SystemBus {
    pub const fn new() -> Self {
        Self {
            vitals: Watch::new(),
            wifi_status: Watch::new(),
            data_channel: Channel::new(),
        }
    }
}
