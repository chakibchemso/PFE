//! Touch device abstraction — production board uses CST9217 on I2C.
//!
//! The CST9217 reports touch via an active‑low INT pin (GPIO11). The touch
//! task waits for this edge instead of polling, reducing bus traffic and CPU
//! wakeups when the panel is idle.

use embassy_time::Delay;

use crate::drivers::bus::{SharedI2cBus, SharedI2cDevice};
use crate::drivers::low::cst9217::{Cst9217, Error as CstError};
use crate::ui::config::RenderConfig;

/// I²C error type for the shared bus device.
pub type SharedI2cError =
    embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>;

pub type Cst9217Touch = Cst9217<SharedI2cDevice, Delay, esp_hal::gpio::Output<'static>>;

pub enum TouchDevice {
    Cst9217(Cst9217Touch),
}

impl TouchDevice {
    /// Initialise the CST9217 on the shared I2C bus.
    ///
    /// The reset pin should already be high. The driver will handle the
    /// reset sequence internally via [`Cst9217::init`].
    pub async fn new(
        i2c_bus: &'static SharedI2cBus,
        delay: Delay,
        touch_rst: esp_hal::gpio::Output<'static>,
        config: &RenderConfig,
    ) -> Result<Self, CstError<SharedI2cError>> {
        let i2c = SharedI2cDevice::new(i2c_bus);

        let mut touch = Cst9217::new(i2c, delay, Some(touch_rst), 0x5A);
        touch.init().await?;

        // Apply touch coordinate transformations from render config
        touch.set_swap_xy(config.touch_swap_xy);
        touch.set_mirror_xy(config.touch_mirror_x, config.touch_mirror_y);
        touch.set_max_coordinates(config.panel_width, config.panel_height);

        Ok(Self::Cst9217(touch))
    }

    pub async fn read_touch(&mut self) -> Result<Option<(u16, u16)>, CstError<SharedI2cError>> {
        match self {
            Self::Cst9217(touch) => {
                let data = touch.read_touch().await?;
                Ok(data.points[0].map(|p| (p.x, p.y)))
            }
        }
    }
}
