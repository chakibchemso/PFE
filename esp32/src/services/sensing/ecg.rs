//! ECG service — initialises the MAX30003 and spawns acquisition.
//!
//! This module provides [`init_sensor`] which sets up the MAX30003 and returns
//! a ready-to-run [`EcgRunner`]. The caller must wrap it in a concrete embassy
//! task (embassy 0.10 does not support generic tasks).

use crate::drivers::ecg::{EcgHandle, EcgRunner};
use crate::drivers::low::max30003::Max30003;

/// Initialise the MAX30003 sensor and return a ready-to-run [`EcgRunner`].
///
/// The caller is responsible for spawning the runner inside a concrete
/// `#[embassy_executor::task]` wrapper.
pub async fn init_sensor<SPI>(sensor: Max30003<SPI>) -> EcgRunner<SPI>
where
    SPI: embedded_hal_async::spi::SpiDevice,
{
    let mut delay = embassy_time::Delay;
    EcgHandle::start(sensor, &mut delay).await
}

/// Re-export the ECG acquisition task helper for use in concrete wrappers.
pub use crate::drivers::ecg::ecg_acquisition_task;

/// Export ECG sample buffer for UI consumption.
pub use crate::drivers::ecg::ECG_SAMPLE_BUFFER;
