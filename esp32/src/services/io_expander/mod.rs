//! I/O expander service — future home of TCA9554 interrupt monitoring.
//!
//! The shared [`IoExpanderService`] lives in `drivers::io_expander` to
//! avoid circular dependencies (the oxymeter driver needs it).
//!
//! This module will later own a background task that monitors the
//! TCA9554's INT output pin and sends channel notifications when a
//! sensor interrupt fires.

pub use crate::drivers::io_expander::IoExpanderService;
