//! Touch input service — CST9217 driver + LVGL pointer input device.
//!
//! The touch task polls the CST9217 and writes coordinates to shared atomics.
//! LVGL's read callback (registered via [`register_indev`]) reads them back.
//! Both run on core 1; Relaxed ordering is sufficient.

pub mod driver;
pub mod task;

use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

/// Current touch X coordinate in LVGL pixel space.
pub static TOUCH_X: AtomicU16 = AtomicU16::new(0);
/// Current touch Y coordinate in LVGL pixel space.
pub static TOUCH_Y: AtomicU16 = AtomicU16::new(0);
/// True while the panel is being touched.
pub static TOUCH_PRESSED: AtomicBool = AtomicBool::new(false);

/// Register a LVGL pointer input device backed by the touch panel.
///
/// Must be called after `lv_init()`, before any widgets that need touch.
/// Call this from your [`oxivgl::view::View::create`] implementation.
pub fn register_indev() {
    // SAFETY: lv_init() must have been called (caller's responsibility).
    // The indev and read callback are valid for the LVGL display lifetime.
    unsafe {
        let indev = oxivgl_sys::lv_indev_create();
        assert!(!indev.is_null(), "lv_indev_create returned NULL");
        oxivgl_sys::lv_indev_set_type(indev, oxivgl_sys::lv_indev_type_t_LV_INDEV_TYPE_POINTER);
        oxivgl_sys::lv_indev_set_read_cb(indev, Some(touch_read_cb));
    }
}

unsafe extern "C" fn touch_read_cb(
    _indev: *mut oxivgl_sys::lv_indev_t,
    data: *mut oxivgl_sys::lv_indev_data_t,
) {
    unsafe {
        if TOUCH_PRESSED.load(Ordering::Relaxed) {
            (*data).point.x = TOUCH_X.load(Ordering::Relaxed) as i32;
            (*data).point.y = TOUCH_Y.load(Ordering::Relaxed) as i32;
            (*data).state = oxivgl_sys::lv_indev_state_t_LV_INDEV_STATE_PRESSED;
        } else {
            (*data).state = oxivgl_sys::lv_indev_state_t_LV_INDEV_STATE_RELEASED;
        }
    }
}
