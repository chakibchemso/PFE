//! Sensor task: oxymeter acquisition task re-export.
//!
//! The actual DSP and acquisition logic lives in `crate::drivers::oxymeter`.
//! This module re-exports the task for organized access via `tasks::sensor`.

pub use crate::drivers::oxymeter::OxymeterHandle;
pub use crate::drivers::oxymeter::OxymeterRunner;
pub use crate::drivers::oxymeter::acquisition_task;
