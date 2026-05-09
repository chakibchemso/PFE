//! Touch device abstraction — production board uses CST9217 on I2C.
//!
//! The CST9217 reports touch via an active‑low INT pin (GPIO11).  The touch
//! task waits for this edge instead of polling, reducing bus traffic and CPU
//! wakeups when the panel is idle.

use embassy_time::Timer;

use crate::drivers::bus::{SharedI2cBus, SharedI2cDevice};
use crate::drivers::low::cst9217::Cst9217;

pub type Cst9217Touch = Cst9217<SharedI2cDevice>;

pub enum TouchDevice {
    Cst9217(Cst9217Touch),
}

impl TouchDevice {
    /// Initialise the CST9217 on the shared I2C bus.
    ///
    /// `touch_rst` must already be de‑asserted (high) by the caller before
    /// this function is called.
    pub async fn new(i2c_bus: &'static SharedI2cBus) -> Result<Self, TouchInitError> {
        let i2c = SharedI2cDevice::new(i2c_bus);

        // Wait 10 ms after reset de‑assert for the chip to boot
        Timer::after_millis(10).await;

        let mut touch = Cst9217::new(i2c);

        // Flush any stale interrupt by attempting a read
        let _ = touch.read_touch().await;

        Ok(Self::Cst9217(touch))
    }

    pub async fn read_touch(&mut self) -> Result<Option<(u16, u16)>, TouchReadError> {
        match self {
            Self::Cst9217(touch) => {
                let point = touch.read_touch().await.map_err(TouchReadError::Cst9217)?;
                Ok(point.map(|p| (p.x, p.y)))
            }
        }
    }
}

#[derive(Debug)]
pub enum TouchInitError {
    I2c(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
}

#[derive(Debug)]
pub enum TouchReadError {
    Cst9217(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
}
