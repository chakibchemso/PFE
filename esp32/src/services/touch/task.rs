use defmt::{error, info, trace};
use embassy_time::{Delay, Duration, Instant, Timer, with_timeout};
use esp_hal::gpio::Output;

use super::driver::TouchDevice;
use super::{TOUCH_PRESSED, TOUCH_X, TOUCH_Y};
use crate::drivers::bus::I2cPeripheral;
use crate::ui::config::RenderConfig;

/// Consecutive empty reads before clearing pressed state (~30 ms).
const RELEASE_DEBOUNCE: u8 = 2;

/// Consecutive identical-coordinate reads before forcing release (~2 s at 15 ms).
const STALE_LIMIT: u16 = 133;

/// CST9217 reads are small; anything beyond this means the I2C transaction or
/// shared bus lock is wedged long enough to break input responsiveness.
const TOUCH_READ_TIMEOUT: Duration = Duration::from_millis(50);

#[embassy_executor::task]
pub async fn touch_task(i2c: I2cPeripheral, touch_rst: Output<'static>, config: RenderConfig) {
    let delay = Delay;

    let mut device = TouchDevice::new(i2c, delay, touch_rst, &config)
        .await
        .expect("Failed to initialize touch controller");

    let mut release_debounce: u8 = 0;
    let mut stale_count: u16 = 0;
    let mut last_x: u16 = 0;
    let mut last_y: u16 = 0;
    let mut stale_logged: bool = false;

    loop {
        Timer::after(Duration::from_millis(15)).await;
        trace!("T: tick");

        let read_start = Instant::now();
        let read_result = with_timeout(TOUCH_READ_TIMEOUT, device.read_touch()).await;
        let read_elapsed = read_start.elapsed().as_millis();
        if read_elapsed > 25 {
            info!("Touch read slow: {} ms", read_elapsed);
        }

        match read_result {
            Err(_) => {
                info!("Touch read timeout");
                TOUCH_PRESSED.store(false, core::sync::atomic::Ordering::Relaxed);
            }
            Ok(result) => match result {
                Ok(Some((x, y))) => {
                    if x == last_x && y == last_y {
                        stale_count = stale_count.saturating_add(1);
                        if stale_count >= STALE_LIMIT {
                            if !stale_logged {
                                info!("Touch stuck at ({}, {}) — forcing release", x, y);
                                stale_logged = true;
                            }
                            TOUCH_PRESSED.store(false, core::sync::atomic::Ordering::Relaxed);
                            continue;
                        }
                    } else {
                        stale_count = 0;
                        stale_logged = false;
                    }

                    if let Some((lx, ly)) = config.map_touch_to_viewport(x, y) {
                        use core::sync::atomic::Ordering;
                        TOUCH_X.store(lx, Ordering::Relaxed);
                        TOUCH_Y.store(ly, Ordering::Relaxed);
                        TOUCH_PRESSED.store(true, Ordering::Relaxed);
                    }
                    last_x = x;
                    last_y = y;
                    release_debounce = 0;
                }
                Ok(None) => {
                    stale_count = 0;
                    stale_logged = false;
                    if release_debounce < RELEASE_DEBOUNCE {
                        release_debounce += 1;
                    } else {
                        TOUCH_PRESSED.store(false, core::sync::atomic::Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    error!("Touch I2C error: {}", defmt::Debug2Format(&e));
                }
            },
        }
    }
}
