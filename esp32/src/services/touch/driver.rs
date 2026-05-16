//! Touch device abstraction — production board uses CST9217 on I2C.
//!
//! The CST9217 reports touch via an active‑low INT pin (GPIO11). The touch
//! task waits for this edge instead of polling, reducing bus traffic and CPU
//! wakeups when the panel is idle.

use embassy_time::{Delay, Duration, Timer};

use crate::drivers::bus::{BusError, I2cPeripheral};
use crate::drivers::low::cst9217::{Cst9217, Error as CstError};
use crate::ui::config::RenderConfig;

pub type Cst9217Touch = Cst9217<I2cPeripheral, Delay, esp_hal::gpio::Output<'static>>;

/// Number of init retry attempts before giving up.
const INIT_RETRIES: u8 = 3;

pub enum TouchDevice {
    Cst9217(Cst9217Touch),
}

impl TouchDevice {
    /// Initialise the CST9217 on the given I²C device handle.
    ///
    /// The reset pin should already be high. The driver handles the reset
    /// sequence via [`Cst9217::init`] with retry / per-device recovery.
    pub async fn new(
        i2c: I2cPeripheral,
        delay: Delay,
        touch_rst: esp_hal::gpio::Output<'static>,
        config: &RenderConfig,
    ) -> Result<Self, CstError<BusError>> {
        // Create the driver once — delay and touch_rst are not Copy, so
        // we can't recreate them inside a retry loop. Retry is handled
        // on init() which resets the chip each time.
        let mut recovery = i2c.clone();
        let mut touch = Cst9217::new(i2c, delay, Some(touch_rst), 0x5A);

        let mut last_err = None;
        for attempt in 0..INIT_RETRIES {
            match touch.init().await {
                Ok(_) => {
                    touch.set_swap_xy(config.touch_swap_xy);
                    touch.set_mirror_xy(config.touch_mirror_x, config.touch_mirror_y);
                    touch.set_max_coordinates(config.panel_width, config.panel_height);
                    return Ok(Self::Cst9217(touch));
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < INIT_RETRIES - 1 {
                        recovery.recover().await;
                        Timer::after(Duration::from_millis(10)).await;
                    }
                }
            }
        }

        Err(last_err.unwrap())
    }

    pub async fn read_touch(&mut self) -> Result<Option<(u16, u16)>, CstError<BusError>> {
        match self {
            Self::Cst9217(touch) => {
                let data = touch.read_touch().await?;
                Ok(data.points[0].map(|p| (p.x, p.y)))
            }
        }
    }
}
