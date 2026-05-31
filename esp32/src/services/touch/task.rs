//! Touch polling task — reads CST9217 over I²C every 15 ms.
//!
//! The CST9217 driver already decodes finger down/up via the event
//! byte in the hardware frame (0x06 = down, 0x00 = up).  We just poll
//! `read_touch()` directly — no INT-pin gating needed.  A small
//! debounce prevents flicker on transient I²C glitches.

use defmt::info;
use embassy_time::{Delay, Duration, Timer};
use esp_hal::gpio::Output;

use super::driver::TouchDevice;
use super::{TOUCH_PRESSED, TOUCH_X, TOUCH_Y};
use crate::drivers::bus::I2cPeripheral;
use crate::ui::config::RenderConfig;

/// Consecutive empty reads before clearing pressed state (~30 ms).
const RELEASE_DEBOUNCE: u8 = 2;

/// Touch task: initialises CST9217, then polls touch data and writes to shared atomics.
#[embassy_executor::task]
pub async fn touch_task(
    i2c: I2cPeripheral,
    touch_rst: Output<'static>,
    config: RenderConfig,
) {
    let delay = Delay;

    let mut device = TouchDevice::new(i2c, delay, touch_rst, &config)
        .await
        .expect("Failed to initialize touch controller");

    let mut release_debounce: u8 = 0;

    loop {
        match device.read_touch().await {
            Ok(Some((x, y))) => {
                if let Some((lx, ly)) = config.map_touch_to_viewport(x, y) {
                    use core::sync::atomic::Ordering;
                    TOUCH_X.store(lx, Ordering::Relaxed);
                    TOUCH_Y.store(ly, Ordering::Relaxed);
                    TOUCH_PRESSED.store(true, Ordering::Relaxed);
                }
                release_debounce = 0;
            }
            Ok(None) => {
                if release_debounce < RELEASE_DEBOUNCE {
                    release_debounce += 1;
                } else {
                    TOUCH_PRESSED.store(false, core::sync::atomic::Ordering::Relaxed);
                }
            }
            Err(e) => {
                info!("Touch I2C error: {}", defmt::Debug2Format(&e));
            }
        }

        Timer::after(Duration::from_millis(15)).await;
    }
}
