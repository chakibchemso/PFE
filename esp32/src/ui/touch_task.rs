use alloc::rc::Rc;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{PointerEventButton, WindowEvent};

use crate::drivers::touch::TouchDevice;

use super::config::RenderConfig;

/// Shared window handle type
pub type SharedWindowHandle =
    Mutex<CriticalSectionRawMutex, core::cell::RefCell<Option<Rc<MinimalSoftwareWindow>>>>;

/// Touch task: polls the touch controller and dispatches events
/// directly to the Slint window, independent of the UI render loop.
#[embassy_executor::task]
pub async fn touch_task(
    device: TouchDevice,
    config: RenderConfig,
    window_ref: &'static SharedWindowHandle,
) {
    let mut device = device;
    let mut last_had_touch = false;
    let mut last_position: Option<slint::LogicalPosition> = None;

    loop {
        let result = device.read_touch().ok().flatten();

        // Get window if available (UI task may not have registered it yet)
        let window_opt = {
            let guard = window_ref.lock().await;
            guard.borrow().clone()
        };

        match (last_had_touch, result) {
            (false, Some((x, y))) => {
                last_had_touch = true;
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
                last_had_touch = false;
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
            (false, None) => {}
        }

        Timer::after(Duration::from_millis(8)).await;
    }
}
