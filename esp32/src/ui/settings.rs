use core::ffi::CStr;
use core::sync::atomic::Ordering;

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::support::Align;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::{Button, Label, Wdg};

use super::display_settings::display_settings_overlay;
use super::gmt::{LAST_GMT_OFFSET, gmt_overlay};
use super::keyboard::keyboard_overlay;

pub fn create(parent: &mut Wdg) {
    let pw = crate::ui::config::PRODUCTION_UI_SIZE as i32;
    let parent_raw = parent.raw_mut();

    // ── Title ────────────────────────────────────────────────────────────
    let mut lbl = Label::new();
    lbl.set_text(cstr!("Settings"));
    align_to(&mut lbl, parent, Align::TopMid, 0, 25);
    lbl.set_parent(parent);
    let _ = lbl.leak();

    // ── Config rows ──────────────────────────────────────────────────────
    let mut row_y = 90;
    let row_h = 50;

    macro_rules! config_row {
        ($label:expr, $glyph:expr) => {{
            let mut row = Button::new();
            row.set_size(pw - 100, row_h);
            unsafe {
                lv_obj_set_parent(row.raw_mut(), parent_raw);
                lv_obj_set_pos(row.raw_mut(), 50, row_y);
                lv_obj_set_style_radius(row.raw_mut(), LV_RADIUS_CIRCLE as i32, 0);
            }
            let mut rl = Label::new();
            rl.set_text($label);
            rl.set_parent(&mut row);
            unsafe {
                lv_obj_align(rl.raw_mut(), lv_align_t_LV_ALIGN_LEFT_MID, 12, 0);
            }
            let mut gl = Label::new();
            gl.set_text(cstr!($glyph));
            unsafe {
                lv_obj_set_parent(gl.raw_mut(), row.raw_mut());
                lv_obj_align(gl.raw_mut(), lv_align_t_LV_ALIGN_RIGHT_MID, -12, 0);
            }
            let _ = gl.leak();
            let _ = rl.leak();
            let row_raw = row.raw_mut();
            let _ = row.leak();
            row_y += row_h + 4;
            row_raw
        }};
    }

    // ── Display (theme + brightness) ─────────────────────────────────────
    let disp_row = config_row!(cstr!("Display"), ">");
    unsafe {
        lv_obj_add_event_cb(
            disp_row,
            Some(display_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── WiFi SSID ────────────────────────────────────────────────────────
    let ssid_row = config_row!(cstr!("WiFi SSID"), ">");
    unsafe {
        lv_obj_add_event_cb(
            ssid_row,
            Some(ssid_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── WiFi Password ────────────────────────────────────────────────────
    let pwd_row = config_row!(cstr!("WiFi Password"), ">");
    unsafe {
        lv_obj_add_event_cb(
            pwd_row,
            Some(pwd_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── Ascon Key ────────────────────────────────────────────────────────
    let ascon_row = config_row!(cstr!("Crypto Key"), ">");
    unsafe {
        lv_obj_add_event_cb(
            ascon_row,
            Some(ascon_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── GMT Offset ──────────────────────────────────────────────────────
    let gmt_row = config_row!(cstr!("GMT Offset"), ">");
    unsafe {
        lv_obj_add_event_cb(
            gmt_row,
            Some(gmt_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }
}

// ── Click callbacks ──────────────────────────────────────────────────────

unsafe extern "C" fn display_click_cb(e: *mut lv_event_t) {
    unsafe {
        let parent = lv_event_get_user_data(e) as *mut lv_obj_t;
        display_settings_overlay(parent);
    }
}

unsafe extern "C" fn ssid_click_cb(e: *mut lv_event_t) {
    unsafe {
        let parent = lv_event_get_user_data(e) as *mut lv_obj_t;
        keyboard_overlay(
            parent,
            cstr!("WiFi SSID"),
            "",
            cstr!("Enter SSID"),
            false,
            32,
            crate::services::storage::KEY_WIFI_SSID,
        );
    }
}

unsafe extern "C" fn pwd_click_cb(e: *mut lv_event_t) {
    unsafe {
        let parent = lv_event_get_user_data(e) as *mut lv_obj_t;
        keyboard_overlay(
            parent,
            cstr!("WiFi Password"),
            "",
            cstr!("Enter password"),
            true,
            64,
            crate::services::storage::KEY_WIFI_PASSWD,
        );
    }
}

unsafe extern "C" fn ascon_click_cb(e: *mut lv_event_t) {
    unsafe {
        let parent = lv_event_get_user_data(e) as *mut lv_obj_t;
        keyboard_overlay(
            parent,
            cstr!("Crypto Key (16 chars)"),
            "",
            cstr!("16 ASCII chars"),
            true,
            16,
            crate::services::storage::KEY_ASCON,
        );
    }
}

unsafe extern "C" fn gmt_click_cb(e: *mut lv_event_t) {
    unsafe {
        let parent = lv_event_get_user_data(e) as *mut lv_obj_t;
        let off = LAST_GMT_OFFSET.load(Ordering::Relaxed);
        gmt_overlay(parent, off);
    }
}

/// Align a widget relative to its parent with offset.
fn align_to(w: &mut Wdg, _parent: &Wdg, align: Align, x_ofs: i32, y_ofs: i32) {
    let a: lv_bevy_ecs::sys::lv_align_t = align.into();
    unsafe {
        lv_bevy_ecs::sys::lv_obj_align(w.raw_mut(), a, x_ofs, y_ofs);
    }
}
