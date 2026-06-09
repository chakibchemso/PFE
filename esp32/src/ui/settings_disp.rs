use core::ffi::CStr;
use core::sync::atomic::{AtomicPtr, Ordering};

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::{Button, Label};

use super::geom::scale;
use super::settings_kb::{lock_tileview, unlock_tileview};
use super::theme::{CURRENT_BRIGHTNESS, CURRENT_THEME, ThemePalette, current_palette};
use crate::services::rendering::task::BRIGHTNESS_CHANNEL;
use crate::services::ui::task::PENDING_SAVES;
use crate::services::ui::task::PendingSave;

// ── Open-modal handles for re-theming ───────────────────────────────────
struct DisplayData {
    overlay: *mut lv_obj_t,
    slider: *mut lv_obj_t,
    theme_sw: *mut lv_obj_t,
    done_btn: *mut lv_obj_t,
    close_btn: *mut lv_obj_t,
}
static DISP_OPEN: AtomicPtr<DisplayData> = AtomicPtr::new(core::ptr::null_mut());

fn apply_theme(d: &DisplayData, pal: &ThemePalette) {
    let bg = lv_color_hex(pal.bg_color);
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);
    let accent = lv_color_hex(pal.accent_color);
    unsafe {
        lv_obj_set_style_bg_color(d.overlay, bg, 0);
        lv_obj_set_style_bg_opa(d.overlay, 255, 0);
        lv_obj_set_style_text_color(d.overlay, text, 0);

        lv_obj_set_style_bg_color(d.theme_sw, overlay, 0);
        lv_obj_set_style_bg_color(
            d.theme_sw,
            accent,
            lv_part_t_LV_PART_INDICATOR | lv_state_t_LV_STATE_CHECKED,
        );
        lv_obj_set_style_bg_opa(
            d.theme_sw,
            255,
            lv_part_t_LV_PART_INDICATOR | lv_state_t_LV_STATE_CHECKED,
        );
        lv_obj_set_style_bg_color(d.theme_sw, text, lv_part_t_LV_PART_KNOB);

        lv_obj_set_style_bg_color(d.slider, overlay, 0);
        lv_obj_set_style_bg_color(d.slider, accent, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_bg_color(d.slider, accent, lv_part_t_LV_PART_KNOB);

        lv_obj_set_style_bg_color(d.done_btn, accent, 0);
        lv_obj_set_style_text_color(d.done_btn, text, 0);
        lv_obj_set_style_radius(d.done_btn, LV_RADIUS_CIRCLE as i32, 0);

        lv_obj_set_style_bg_color(d.close_btn, overlay, 0);
        lv_obj_set_style_text_color(d.close_btn, text, 0);
        lv_obj_set_style_radius(d.close_btn, LV_RADIUS_CIRCLE as i32, 0);
    }
}

pub fn re_theme() {
    let ptr = DISP_OPEN.load(Ordering::Relaxed);
    if !ptr.is_null() {
        apply_theme(unsafe { &*ptr }, current_palette());
    }
}

// ── Modal panel (fullscreen) ────────────────────────────────────────
const MODAL_RADIUS: i32 = 0;

// ── Title label ──────────────────────────────────────────────────────
const TITLE_Y: i32 = scale(8);

// ── Theme row ────────────────────────────────────────────────────────
const THEME_LBL_X: i32 = scale(60);
const THEME_LBL_Y: i32 = scale(80);
const SW_X: i32 = scale(300);
const SW_Y: i32 = scale(75);

// ── Brightness row ───────────────────────────────────────────────────
const BRIGHT_LBL_X: i32 = THEME_LBL_X;
const BRIGHT_LBL_Y: i32 = scale(130);
const SL_X: i32 = scale(60);
const SL_Y: i32 = scale(170);
const SL_W: i32 = scale(290);
const SL_H: i32 = scale(24);

// ── Save / Close buttons ─────────────────────────────────────────────
const BTN_W: i32 = scale(140);
const BTN_H: i32 = scale(40);
const BTN_Y: i32 = scale(320);
const BTN_X: i32 = scale(145);
const CLOSE_BTN_Y: i32 = BTN_Y + BTN_H + scale(10);

unsafe extern "C" fn panel_cb(e: *mut lv_event_t) {
    unsafe {
        lv_event_stop_bubbling(e);
    }
}

unsafe extern "C" fn modal_close_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *const DisplayData;
        DISP_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}

unsafe extern "C" fn theme_switch_cb(e: *mut lv_event_t) {
    unsafe {
        let target = lv_event_get_current_target(e);
        let checked = lv_obj_has_state(target as *const lv_obj_t, lv_state_t_LV_STATE_CHECKED);
        CURRENT_THEME.store(if checked { 1 } else { 0 }, Ordering::Relaxed);
    }
}

unsafe extern "C" fn done_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *const DisplayData;
        let checked = lv_obj_has_state(
            (*data).theme_sw as *const lv_obj_t,
            lv_state_t_LV_STATE_CHECKED,
        );
        let brightness = lv_slider_get_value((*data).slider as *const lv_obj_t) as u8;
        let _ = PENDING_SAVES.try_send(PendingSave {
            key: crate::services::storage::KEY_THEME,
            data: alloc::vec![if checked { 1 } else { 0 }],
        });
        let _ = PENDING_SAVES.try_send(PendingSave {
            key: crate::services::storage::KEY_BRIGHTNESS,
            data: alloc::vec![brightness],
        });
        DISP_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}

unsafe extern "C" fn bright_slider_cb(e: *mut lv_event_t) {
    unsafe {
        let target = lv_event_get_current_target(e);
        let val = lv_slider_get_value(target as *const lv_obj_t);
        let brightness = ((val.max(0) as u16 * 255 + 50) / 100).min(255) as u8;
        let _ = BRIGHTNESS_CHANNEL.try_send(brightness);
    }
}

pub fn display_settings_overlay(parent: *mut lv_obj_t) {
    let pal = current_palette();
    let screen = unsafe { lv_screen_active() };
    let pw = unsafe { lv_obj_get_width(screen) };
    let ph = unsafe { lv_obj_get_height(screen) };

    // Lock tileview scrolling
    unsafe { lock_tileview() }

    // ── Fullscreen modal panel (on active screen) ────────────────────
    let panel = unsafe { lv_obj_create(screen) };
    unsafe {
        lv_obj_set_size(panel, pw, ph);
        lv_obj_set_pos(panel, 0, 0);
        lv_obj_remove_flag(panel, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        lv_obj_set_style_border_side(panel, lv_border_side_t_LV_BORDER_SIDE_NONE, 0);
        lv_obj_set_style_radius(panel, MODAL_RADIUS, 0);
        lv_obj_add_event_cb(
            panel,
            Some(panel_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            core::ptr::null_mut(),
        );
    }

    // ── Title ────────────────────────────────────────────────────────
    let mut title_lbl = Label::new();
    title_lbl.set_text(cstr!("Display"));
    unsafe {
        lv_obj_set_parent(title_lbl.raw_mut(), panel);
        lv_obj_align(title_lbl.raw_mut(), lv_align_t_LV_ALIGN_TOP_MID, 0, TITLE_Y);
    }
    let _ = title_lbl.leak();

    // ── Theme switch ─────────────────────────────────────────────────
    let mut theme_lbl = Label::new();
    theme_lbl.set_text(cstr!("Theme"));
    unsafe {
        lv_obj_set_parent(theme_lbl.raw_mut(), panel);
        lv_obj_set_pos(theme_lbl.raw_mut(), THEME_LBL_X, THEME_LBL_Y);
    }
    let _ = theme_lbl.leak();

    let sw = unsafe { lv_switch_create(panel) };
    unsafe {
        lv_obj_set_pos(sw, SW_X, SW_Y);
        // lv_obj_set_size(sw, 40, 20);
    }
    if CURRENT_THEME.load(Ordering::Relaxed) == 1 {
        unsafe { lv_obj_add_state(sw, lv_state_t_LV_STATE_CHECKED) };
    }
    unsafe {
        lv_obj_add_event_cb(
            sw,
            Some(theme_switch_cb),
            lv_event_code_t_LV_EVENT_VALUE_CHANGED,
            core::ptr::null_mut(),
        );
    }

    // ── Brightness slider ────────────────────────────────────────────
    let mut bright_lbl = Label::new();
    bright_lbl.set_text(cstr!("Brightness"));
    unsafe {
        lv_obj_set_parent(bright_lbl.raw_mut(), panel);
        lv_obj_set_pos(bright_lbl.raw_mut(), BRIGHT_LBL_X, BRIGHT_LBL_Y);
    }
    let _ = bright_lbl.leak();

    let sl = unsafe { lv_slider_create(panel) };
    unsafe {
        lv_slider_set_range(sl, 5, 100);
        lv_slider_set_value(sl, CURRENT_BRIGHTNESS.load(Ordering::Relaxed) as i32, false);
        lv_obj_set_pos(sl, SL_X, SL_Y);
        lv_obj_set_size(sl, SL_W, SL_H);
        // lv_obj_set_style_radius(sl, LV_RADIUS_CIRCLE as i32, 0);
    }
    unsafe {
        lv_obj_add_event_cb(
            sl,
            Some(bright_slider_cb),
            lv_event_code_t_LV_EVENT_VALUE_CHANGED,
            core::ptr::null_mut(),
        );
    }

    // ── Done button (saves to storage) ──────────────────────────────
    let mut done_btn = Button::new();
    unsafe {
        lv_obj_set_parent(done_btn.raw_mut(), panel);
        lv_obj_set_pos(done_btn.raw_mut(), BTN_X, BTN_Y);
        lv_obj_set_size(done_btn.raw_mut(), BTN_W, BTN_H);
    }
    let mut done_lbl = Label::new();
    done_lbl.set_text(cstr!("Save"));
    unsafe {
        lv_obj_set_parent(done_lbl.raw_mut(), done_btn.raw_mut());
        lv_obj_center(done_lbl.raw_mut());
    }
    let _ = done_lbl.leak();
    let done_raw = done_btn.raw_mut();
    let _ = done_btn.leak();

    // ── Close button (below done) ────────────────────────────────────
    let mut close_btn = Button::new();
    unsafe {
        lv_obj_set_parent(close_btn.raw_mut(), panel);
        lv_obj_set_pos(close_btn.raw_mut(), BTN_X, CLOSE_BTN_Y);
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

    // ── Wire up event callbacks ──────────────────────────────────────
    let display_data: &'static mut DisplayData =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(DisplayData {
            overlay: panel,
            slider: sl,
            theme_sw: sw,
            done_btn: done_raw,
            close_btn: close_raw,
        }));

    unsafe {
        lv_obj_add_event_cb(
            done_raw,
            Some(done_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            display_data as *mut DisplayData as *mut _,
        );
        lv_obj_add_event_cb(
            close_raw,
            Some(modal_close_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            display_data as *mut DisplayData as *mut _,
        );
    }

    // ── Apply theme colors ───────────────────────────────────────────
    DISP_OPEN.store(display_data as *mut DisplayData, Ordering::Relaxed);
    apply_theme(display_data, pal);
}
