//! Shared state and communication channels between tasks.
//!
//! This module owns all global synchronization primitives:
//! - `DATA_CHANNEL`: encrypted sensor data → MQTT task
//! - `VITALS_CHANNEL`: raw vitals (BPM, SpO2, temp) → UI task
//!
//! Add settings, event bus, and global state here as the project grows.

use alloc::vec::Vec;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::{Receiver, Watch};

/// Channel for sending encrypted payloads from pipeline_task to mqtt_task.
pub static DATA_CHANNEL: Channel<CriticalSectionRawMutex, Vec<u8>, 5> = Channel::new();

/// Watch channel for broadcasting vitals (BPM, SpO2, temp) to UI and other consumers.
pub static VITALS_CHANNEL: Watch<CriticalSectionRawMutex, (f32, f32, f32), 2> = Watch::new();

/// Get a new receiver for the vitals channel.
pub fn vitals_receiver() -> Receiver<'static, CriticalSectionRawMutex, (f32, f32, f32), 2> {
    VITALS_CHANNEL
        .receiver()
        .expect("Failed to get vitals receiver")
}
