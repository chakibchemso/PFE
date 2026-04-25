#![no_std]
pub mod crypto;
pub mod init;
pub mod mqtt;
pub mod oxymeter;
pub mod plotter;
pub mod processor;
pub mod touch;
pub mod ui;
pub mod utils;
pub mod wifi;

pub extern crate alloc;
pub extern crate panic_rtt_target;
pub extern crate rtt_target;

use alloc::vec::Vec;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

// Create a global channel for sending encrypted data from main to mqtt task
pub static DATA_CHANNEL: Channel<CriticalSectionRawMutex, Vec<u8>, 5> = Channel::new();
