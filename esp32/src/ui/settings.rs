use core::sync::atomic::Ordering;

use oxivgl::enums::{EventCode, ObjState};
use oxivgl::event::Event;
use oxivgl::widgets::*;

use super::theme::CURRENT_THEME;
use crate::services::rendering::task::{BRIGHTNESS_CHANNEL, SCREEN_H, SCREEN_W};

pub struct Handles {
    pub theme_switch: *mut oxivgl_sys::lv_obj_t,
    pub bright_slider: *mut oxivgl_sys::lv_obj_t,
}

/// Navigate to the watchface (tile index 1,0). The tileview handle is
/// captured in a static for access from the LVGL event callback.
static TILEVIEW_HANDLE: core::sync::atomic::AtomicPtr<core::ffi::c_void> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

pub fn create(
    parent: &impl AsLvHandle,
    tileview_handle: *mut oxivgl_sys::lv_obj_t,
) -> Result<Handles, WidgetError> {
    TILEVIEW_HANDLE.store(tileview_handle as *mut core::ffi::c_void, Ordering::Relaxed);

    let list = List::new(parent)?;
    list.size(SCREEN_W, SCREEN_H).bg_opa(0);
    let _ = Child::new(list);

    // Back button — settings pane has ScrollDir::NONE so the tileview
    // doesn't intercept indev events; navigation is button-driven.
    let back = Button::new(parent)?;
    back.size(80, 40).align(Align::TopLeft, 10, 10);
    let _back_lbl = Label::new(&back)?;
    _back_lbl.text("\u{2190} Back");
    back.on(EventCode::CLICKED, on_back_clicked);
    let _ = Child::new(back);

    let lbl = Label::new(parent)?;
    lbl.text("Settings").align(Align::TopMid, 0, 10);
    let _ = Child::new(lbl);

    let tl = Label::new(parent)?;
    tl.text("Theme").align(Align::Default, 20, 60);
    let _ = Child::new(tl);

    let sw = Switch::new(parent)?;
    sw.align(Align::Default, 360, 55);
    sw.on(EventCode::VALUE_CHANGED, on_theme_toggle);
    let sw_h = sw.handle();
    let _ = Child::new(sw);

    let bl = Label::new(parent)?;
    bl.text("Brightness").align(Align::Default, 20, 120);
    let _ = Child::new(bl);

    let sl = Slider::new(parent)?;
    sl.set_range(0, 100).set_value(80);
    sl.on(EventCode::VALUE_CHANGED, on_brightness_change);
    sl.size(SCREEN_W - 80, 20).align(Align::Default, 40, 120);
    let sl_h = sl.handle();
    let _ = Child::new(sl);

    Ok(Handles { theme_switch: sw_h, bright_slider: sl_h })
}

fn on_back_clicked(_ev: &Event) {
    let h = TILEVIEW_HANDLE.load(Ordering::Relaxed) as *mut oxivgl_sys::lv_obj_t;
    if !h.is_null() {
        unsafe { oxivgl_sys::lv_tileview_set_tile_by_index(h, 1, 0, true) };
    }
}

fn on_theme_toggle(ev: &Event) {
    let h = ev.current_target_handle();
    if !h.is_null() {
        let obj = Obj::from_raw_non_owning(h);
        let checked = obj.has_state(ObjState::CHECKED);
        CURRENT_THEME.store(if checked { 1 } else { 0 }, Ordering::Relaxed);
    }
}

fn on_brightness_change(ev: &Event) {
    let h = ev.current_target_handle();
    if !h.is_null() {
        let val = unsafe { oxivgl_sys::lv_slider_get_value(h) };
        let brightness = ((val.max(0) as u16 * 255 + 50) / 100).min(255) as u8;
        let _ = BRIGHTNESS_CHANNEL.try_send(brightness);
    }
}
