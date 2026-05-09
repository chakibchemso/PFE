//! Application-level constructs: System Bus and shared types.
//!
//! The System Bus (`bus.rs`) is the central IPC manifest — all inter-service
//! channels are defined there. Services extract only the senders/receivers
//! they need.

pub mod bus;
