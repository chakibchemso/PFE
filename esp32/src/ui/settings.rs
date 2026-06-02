use core::ffi::CStr;
use core::ptr::NonNull;
use core::sync::atomic::Ordering;

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::events::EventCode;
use lv_bevy_ecs::support::Align;
use lv_bevy_ecs::sys::{lv_event_get_current_target, lv_obj_add_state, lv_obj_has_state, lv_obj_t};
use lv_bevy_ecs::sys::{lv_state_t_LV_STATE_CHECKED, lv_tileview_set_tile_by_index};
use lv_bevy_ecs::widgets::{Button, Label, Slider, Switch, Wdg};

use super::theme::CURRENT_THEME;
use crate::services::rendering::task::{BRIGHTNESS_CHANNEL, SCREEN_W};

/// Tileview handle for the Back button — set by layout before creating panes.
static TV: core::sync::atomic::AtomicPtr<lv_obj_t> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

pub fn set_tileview_handle(h: *mut lv_obj_t) {
    TV.store(h, Ordering::Relaxed);
}

pub struct Handles {
    pub theme_switch: NonNull<lv_obj_t>,
    pub bright_slider: NonNull<lv_obj_t>,
}

pub fn create(parent: &mut Wdg) -> Handles {
    // Back button
    let mut back = Button::new();
    back.set_size(80, 40);
    align_to(&mut back, parent, Align::TopLeft, 10, 10);
    back.set_parent(parent);

    let mut back_lbl = Label::new();
    back_lbl.set_text(cstr!("< Back"));
    back_lbl.set_parent(&mut back);

    back.add_event_cb(EventCode::Clicked, |_event| {
        let tv = TV.load(Ordering::Relaxed);
        if !tv.is_null() {
            unsafe { lv_tileview_set_tile_by_index(tv, 1, 1, true) };
        }
    });
    let _ = back.leak();
    let _ = back_lbl.leak();

    // Title
    let mut lbl = Label::new();
    lbl.set_text(cstr!("Settings"));
    align_to(&mut lbl, parent, Align::TopMid, 0, 10);
    lbl.set_parent(parent);
    let _ = lbl.leak();

    // Theme label
    let mut tl = Label::new();
    tl.set_text(cstr!("Theme"));
    tl.set_pos(20, 60);
    tl.set_parent(parent);
    let _ = tl.leak();

    // Theme switch
    let mut sw = Switch::new();
    align_to(&mut sw, parent, Align::TopRight, -70, 55);
    sw.set_parent(parent);
    if CURRENT_THEME.load(Ordering::Relaxed) == 1 {
        unsafe { lv_obj_add_state(sw.raw_mut(), lv_state_t_LV_STATE_CHECKED) };
    }
    sw.add_event_cb(EventCode::ValueChanged, |mut event| {
        let target = unsafe { lv_event_get_current_target(event.raw_mut()) };
        if !target.is_null() {
            let checked =
                unsafe { lv_obj_has_state(target as *const lv_obj_t, lv_state_t_LV_STATE_CHECKED) };
            CURRENT_THEME.store(if checked { 1 } else { 0 }, Ordering::Relaxed);
        }
    });
    let sw_h = NonNull::new(sw.raw_mut()).expect("switch handle");
    let _ = sw.leak();

    // Brightness label
    let mut bl = Label::new();
    bl.set_text(cstr!("Brightness"));
    bl.set_pos(20, 120);
    bl.set_parent(parent);
    let _ = bl.leak();

    // Brightness slider
    let mut sl = Slider::new();
    sl.set_range(0, 100);
    sl.set_value(80, false);
    sl.set_size((SCREEN_W - 80).into(), 20);
    align_to(&mut sl, parent, Align::TopLeft, 40, 120);
    sl.set_parent(parent);
    sl.add_event_cb(EventCode::ValueChanged, |mut event| {
        let target = unsafe { lv_event_get_current_target(event.raw_mut()) };
        if !target.is_null() {
            let val = unsafe { lv_bevy_ecs::sys::lv_slider_get_value(target as *const lv_obj_t) };
            let brightness = ((val.max(0) as u16 * 255 + 50) / 100).min(255) as u8;
            let _ = BRIGHTNESS_CHANNEL.try_send(brightness);
        }
    });
    let sl_h = NonNull::new(sl.raw_mut()).expect("slider handle");
    let _ = sl.leak();

    Handles {
        theme_switch: sw_h,
        bright_slider: sl_h,
    }
}

/// Align a widget relative to its parent with offset.
/// LVGL v9's `lv_obj_align(obj, align, x_ofs, y_ofs)` aligns
/// relative to the object's parent.
fn align_to(w: &mut Wdg, _parent: &Wdg, align: Align, x_ofs: i32, y_ofs: i32) {
    let a: lv_bevy_ecs::sys::lv_align_t = align.into();
    unsafe {
        lv_bevy_ecs::sys::lv_obj_align(w.raw_mut(), a, x_ofs, y_ofs);
    }
}
