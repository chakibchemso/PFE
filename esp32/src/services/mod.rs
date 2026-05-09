//! Service-oriented modules for the smartwatch OS.
//!
//! Each service follows the same pattern:
//! ```ignore
//! pub fn register(spawner: &Spawner, ...deps..., bus: &SystemBus)
//! ```
//! Services own their tasks, driver wrappers, and UI formatting logic.
//! Cross-service communication goes through `app::bus::SystemBus`.
//! No service imports another service's internals.

pub mod mqtt;
pub mod rendering;
pub mod sensing;
pub mod storage;
pub mod touch;
pub mod wifi;
