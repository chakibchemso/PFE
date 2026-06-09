use defmt::info;
use embassy_time::{Duration, with_timeout};
use esp_hal::gpio::{Input, Output};

use super::driver::TouchDevice;
use super::{TOUCH_PRESSED, TOUCH_X, TOUCH_Y};
use crate::drivers::bus::I2cPeripheral;
use crate::ui::config::RenderConfig;

const TOUCH_READ_TIMEOUT: Duration = Duration::from_millis(50);
const RELEASE_CONFIRM: Duration = Duration::from_millis(10);
const TOUCH_IDLE_TIMEOUT: Duration = Duration::from_millis(200);

#[embassy_executor::task]
pub async fn touch_task(
    i2c: I2cPeripheral,
    touch_rst: Output<'static>,
    config: RenderConfig,
    mut touch_int: Input<'static>,
) {
    let delay = embassy_time::Delay;

    let mut device = TouchDevice::new(i2c, delay, touch_rst, &config)
        .await
        .expect("Failed to initialize touch controller");

    use core::sync::atomic::Ordering;

    loop {
        if with_timeout(TOUCH_IDLE_TIMEOUT, touch_int.wait_for_falling_edge())
            .await
            .is_err()
        {
            if TOUCH_PRESSED.swap(false, Ordering::Relaxed) {
                info!("T: 0");
            }
            continue;
        }

        let touch = with_timeout(TOUCH_READ_TIMEOUT, device.read_touch()).await;
        let touch = match touch {
            Ok(Ok(None)) | Err(_) => {
                info!("T: 0?");
                if with_timeout(RELEASE_CONFIRM, touch_int.wait_for_falling_edge())
                    .await
                    .is_ok()
                {
                    with_timeout(TOUCH_READ_TIMEOUT, device.read_touch()).await
                } else {
                    touch
                }
            }
            other => other,
        };

        match touch {
            Ok(Ok(Some((x, y)))) => {
                info!("T: 1 {} {}", x, y);
                if let Some((lx, ly)) = config.map_touch_to_viewport(x, y) {
                    // LVGL runs at full 466×466 resolution, matching the physical
                    // panel. Touch coordinates map 1:1 with the display.
                    TOUCH_X.store(lx, Ordering::Relaxed);
                    TOUCH_Y.store(ly, Ordering::Relaxed);
                    TOUCH_PRESSED.store(true, Ordering::Relaxed);
                }
            }
            _ => {
                info!("T: 0");
                TOUCH_PRESSED.store(false, Ordering::Relaxed);
            }
        }
    }
}
