//! Touch task — I2C init + interrupt‑driven reads from the CST9217.
//!
//! The CST9217 asserts its INT pin (active low) when touch data is available.
//! This task initializes the controller, then polls the pin at a moderate rate
//! and only initiates I2C reads when the pin signals activity.

use alloc::rc::Rc;
use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Duration, Timer};
use esp_hal::gpio::{Input, Output};
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{PointerEventButton, WindowEvent};

use super::driver::TouchDevice;
use crate::drivers::bus::I2cPeripheral;
use crate::ui::config::RenderConfig;

/// Shared window handle type
pub type SharedWindowHandle =
    Mutex<CriticalSectionRawMutex, RefCell<Option<Rc<MinimalSoftwareWindow>>>>;

/// Touch task: initializes CST9217, then polls INT pin and dispatches to Slint.
#[embassy_executor::task]
pub async fn touch_task(
    i2c: I2cPeripheral,
    window_ref: &'static SharedWindowHandle,
    int_pin: Input<'static>,
    touch_rst: Output<'static>,
    config: RenderConfig,
) {
    let delay = Delay;

    // Initialize the touch controller
    let mut device = TouchDevice::new(i2c, delay, touch_rst, &config)
        .await
        .expect("Failed to initialize touch controller");

    let mut last_had_touch = false;
    let mut last_position: Option<slint::LogicalPosition> = None;
    let mut release_debounce: u8 = 0;

    const RELEASE_DEBOUNCE: u8 = 50;
    let mut ignore_until = embassy_time::Instant::now();

    loop {
        let result = if int_pin.is_low() {
            device.read_touch().await.ok().flatten()
        } else {
            None
        };

        let window_opt = {
            let guard = window_ref.lock().await;
            guard.borrow().clone()
        };

        match (last_had_touch, result) {
            (false, Some((x, y))) => {
                if embassy_time::Instant::now() < ignore_until {
                    continue;
                }
                last_had_touch = true;
                release_debounce = RELEASE_DEBOUNCE;
                if let Some(position) = config.map_touch_to_viewport(x, y) {
                    last_position = Some(position);
                    if let Some(ref window) = window_opt {
                        window
                            .try_dispatch_event(WindowEvent::PointerPressed {
                                position,
                                button: PointerEventButton::Left,
                            })
                            .ok();
                        window.request_redraw();
                    }
                }
            }
            (true, Some((x, y))) => {
                release_debounce = RELEASE_DEBOUNCE;
                if let Some(position) = config.map_touch_to_viewport(x, y) {
                    last_position = Some(position);
                    if let Some(ref window) = window_opt {
                        window
                            .try_dispatch_event(WindowEvent::PointerMoved { position })
                            .ok();
                        window.request_redraw();
                    }
                }
            }
            (true, None) => {
                if release_debounce > 0 {
                    release_debounce -= 1;
                } else {
                    last_had_touch = false;
                    ignore_until = embassy_time::Instant::now() + Duration::from_millis(20);
                    if let Some(position) = last_position.take() {
                        if let Some(ref window) = window_opt {
                            window
                                .try_dispatch_event(WindowEvent::PointerReleased {
                                    position,
                                    button: PointerEventButton::Left,
                                })
                                .ok();
                            window.request_redraw();
                        }
                    }
                }
            }
            (false, None) => {}
        }

        Timer::after(Duration::from_millis(1)).await;
    }
}
