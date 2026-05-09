#![no_std]
pub mod app;
pub mod config;
pub mod crypto;
pub mod drivers;
pub mod dsp;
pub mod plotter;
pub mod services;
pub mod system;
pub mod ui;
pub mod utils;

pub extern crate alloc;
pub extern crate panic_rtt_target;
pub extern crate rtt_target;
