//! MAX30102 driver wrapper.
//!
//! Thin wrapper around `crate::drivers::oxymeter` for the sensing service.
//! Full init sequence and DSP logic lives in `crate::drivers::oxymeter`.

pub use crate::drivers::oxymeter::{OxymeterHandle, OxymeterRunner, acquisition_task};
