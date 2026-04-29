pub mod mqtt;
pub mod sensor;
pub mod wifi;

// Re-export the oxymeter acquisition task from the oxymeter module
// so it's accessible via tasks::sensor
pub use crate::drivers::oxymeter::acquisition_task;
