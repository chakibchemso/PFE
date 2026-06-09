use core::ffi::CStr;
use core::sync::atomic::{AtomicPtr, Ordering};

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::{Button, Label};

use super::geom::scale;
use super::settings_kb::{lock_tileview, unlock_tileview};
use super::theme::{ThemePalette, current_palette};
use crate::services::power::SHUTDOWN_CHANNEL;

struct PowerData {
    overlay: *mut lv_obj_t,
    shutdown_btn: *mut lv_obj_t,
    close_btn: *mut lv_obj_t,
}
static PWR_OPEN: AtomicPtr<PowerData> = AtomicPtr::new(core::ptr::null_mut());

fn apply_theme(d: &PowerData, pal: &ThemePalette) {
    let bg = lv_color_hex(pal.bg_color);
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);
    let danger = lv_color_hex(0xf38ba8);
    unsafe {
        lv_obj_set_style_bg_color(d.overlay, bg, 0);
        lv_obj_set_style_bg_opa(d.overlay, 255, 0);
        lv_obj_set_style_text_color(d.overlay, text, 0);

        lv_obj_set_style_bg_color(d.shutdown_btn, danger, 0);
        lv_obj_set_style_text_color(d.shutdown_btn, bg, 0);
        lv_obj_set_style_radius(d.shutdown_btn, LV_RADIUS_CIRCLE as i32, 0);

        lv_obj_set_style_bg_color(d.close_btn, overlay, 0);
        lv_obj_set_style_text_color(d.close_btn, text, 0);
        lv_obj_set_style_radius(d.close_btn, LV_RADIUS_CIRCLE as i32, 0);
    }
}

pub fn re_theme() {
    let ptr = PWR_OPEN.load(Ordering::Relaxed);
    if !ptr.is_null() {
        apply_theme(unsafe { &*ptr }, current_palette());
    }
}

const TITLE_Y: i32 = scale(8);
const BTN_W: i32 = scale(200);
const BTN_H: i32 = scale(44);
const SHUTDOWN_BTN_Y: i32 = scale(0);
const CLOSE_BTN_Y: i32 = SHUTDOWN_BTN_Y + BTN_H + scale(14);

unsafe extern "C" fn panel_cb(e: *mut lv_event_t) {
    unsafe {
        lv_event_stop_bubbling(e);
    }
}

unsafe extern "C" fn modal_close_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *const PowerData;
        PWR_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}

unsafe extern "C" fn shutdown_click_cb(_e: *mut lv_event_t) {
    let _ = SHUTDOWN_CHANNEL.try_send(());
}

pub fn power_settings_overlay(parent: *mut lv_obj_t) {
    let pal = current_palette();
    let screen = unsafe { lv_screen_active() };
    let pw = unsafe { lv_obj_get_width(screen) };
    let ph = unsafe { lv_obj_get_height(screen) };

    unsafe { lock_tileview() }

    // Fullscreen modal panel (on active screen)
    let panel = unsafe { lv_obj_create(screen) };
    unsafe {
        lv_obj_set_size(panel, pw, ph);
        lv_obj_set_pos(panel, 0, 0);
        lv_obj_remove_flag(panel, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        lv_obj_set_style_border_side(panel, lv_border_side_t_LV_BORDER_SIDE_NONE, 0);
        lv_obj_set_style_radius(panel, 0, 0);
        lv_obj_add_event_cb(
            panel,
            Some(panel_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            core::ptr::null_mut(),
        );
    }

    // Title
    let mut title_lbl = Label::new();
    title_lbl.set_text(cstr!("Power"));
    unsafe {
        lv_obj_set_parent(title_lbl.raw_mut(), panel);
        lv_obj_align(title_lbl.raw_mut(), lv_align_t_LV_ALIGN_TOP_MID, 0, TITLE_Y);
    }
    let _ = title_lbl.leak();

    // Warning text
    let mut warn_lbl = Label::new();
    warn_lbl.set_text(cstr!("Shut down the watch?"));
    unsafe {
        lv_obj_set_parent(warn_lbl.raw_mut(), panel);
        lv_obj_align(
            warn_lbl.raw_mut(),
            lv_align_t_LV_ALIGN_CENTER,
            0,
            scale(-60),
        );
    }
    let _ = warn_lbl.leak();

    // Shutdown button
    let mut shutdown_btn = Button::new();
    unsafe {
        lv_obj_set_parent(shutdown_btn.raw_mut(), panel);
        lv_obj_align(shutdown_btn.raw_mut(), lv_align_t_LV_ALIGN_CENTER, 0, 0);
        lv_obj_set_pos(shutdown_btn.raw_mut(), 0, SHUTDOWN_BTN_Y);
        lv_obj_set_size(shutdown_btn.raw_mut(), BTN_W, BTN_H);
    }
    let mut shutdown_lbl = Label::new();
    shutdown_lbl.set_text(cstr!("Shutdown"));
    unsafe {
        lv_obj_set_parent(shutdown_lbl.raw_mut(), shutdown_btn.raw_mut());
        lv_obj_center(shutdown_lbl.raw_mut());
    }
    let _ = shutdown_lbl.leak();
    let shutdown_raw = shutdown_btn.raw_mut();
    let _ = shutdown_btn.leak();

    // Close button
    let mut close_btn = Button::new();
    unsafe {
        lv_obj_set_parent(close_btn.raw_mut(), panel);
        lv_obj_align(close_btn.raw_mut(), lv_align_t_LV_ALIGN_CENTER, 0, 0);
        lv_obj_set_pos(close_btn.raw_mut(), 0, CLOSE_BTN_Y);
        lv_obj_set_size(close_btn.raw_mut(), BTN_W, BTN_H);
    }
    let mut close_lbl = Label::new();
    close_lbl.set_text(cstr!("Close"));
    unsafe {
        lv_obj_set_parent(close_lbl.raw_mut(), close_btn.raw_mut());
        lv_obj_center(close_lbl.raw_mut());
    }
    let _ = close_lbl.leak();
    let close_raw = close_btn.raw_mut();
    let _ = close_btn.leak();

    // Wire up callbacks
    let power_data: &'static mut PowerData =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(PowerData {
            overlay: panel,
            shutdown_btn: shutdown_raw,
            close_btn: close_raw,
        }));

    unsafe {
        lv_obj_add_event_cb(
            shutdown_raw,
            Some(shutdown_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            core::ptr::null_mut(),
        );
        lv_obj_add_event_cb(
            close_raw,
            Some(modal_close_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            power_data as *mut PowerData as *mut _,
        );
    }

    PWR_OPEN.store(power_data as *mut PowerData, Ordering::Relaxed);
    apply_theme(power_data, pal);
}
