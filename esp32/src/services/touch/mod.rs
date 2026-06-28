//! Touch input service — CST9217 driver + LVGL pointer input device.
//!
//! The touch task polls the CST9217 on **core 0** and writes coordinates to
//! shared atomics. The LVGL input device polls those atomics via
//! `lv_bevy_ecs`'s `InputDevice` on **core 1**. The split ensures that
//! I²C touch reads are never starved when LVGL rendering is busy.
//! Cross-core `Relaxed` ordering is sufficient for the atomics.

pub mod driver;
pub mod task;

use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use embedded_graphics::prelude::Point;
use lv_bevy_ecs::input::{BufferStatus, InputDevice, InputEvent, InputState, Pointer};

/// Current touch X coordinate in LVGL pixel space.
pub static TOUCH_X: AtomicU16 = AtomicU16::new(0);
/// Current touch Y coordinate in LVGL pixel space.
pub static TOUCH_Y: AtomicU16 = AtomicU16::new(0);
/// True while the panel is being touched.
pub static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);

/// Register a LVGL pointer input device backed by the touch panel.
///
/// Must be called after `lv_init()`, before any widgets that need touch.
/// Returns the `InputDevice` which must be kept alive (leaked or stored
/// in a static).
pub fn register_indev() -> InputDevice<Pointer> {
    InputDevice::<Pointer>::new(|| {
        let pressed = TOUCH_PRESSED.load(Ordering::Relaxed);
        InputEvent {
            state: if pressed {
                InputState::Pressed
            } else {
                InputState::Released
            },
            data: Point::new(
                TOUCH_X.load(Ordering::Relaxed) as i32,
                TOUCH_Y.load(Ordering::Relaxed) as i32,
            ),
            status: BufferStatus::Once,
        }
    })
}
