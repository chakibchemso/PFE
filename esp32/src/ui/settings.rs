use core::ffi::CStr;
use core::sync::atomic::{AtomicPtr, Ordering};

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::support::Align;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::{Button, Label, Wdg};

use super::display_settings::display_settings_overlay;
use super::gmt::{LAST_GMT_OFFSET, gmt_overlay};
use super::keyboard::keyboard_overlay;
use super::power_settings::power_settings_overlay;
use super::theme::current_palette;

struct SettingsHandles {
    labels: [*mut lv_obj_t; 6],
    top_fades: [*mut lv_obj_t; 5],
    bot_fades: [*mut lv_obj_t; 5],
}

static SETTINGS_DATA: AtomicPtr<SettingsHandles> = AtomicPtr::new(core::ptr::null_mut());

pub fn re_theme() {
    let ptr = SETTINGS_DATA.load(Ordering::Relaxed);
    if ptr.is_null() {
        return;
    }
    let d = unsafe { &*ptr };
    let pal = current_palette();
    let text = lv_color_hex(pal.alt_text);
    let bg = lv_color_hex(pal.bg_color);

    for &label in &d.labels {
        unsafe {
            lv_obj_set_style_text_color(label, text, 0);
        }
    }
    for &bar in &d.top_fades {
        unsafe {
            lv_obj_set_style_bg_color(bar, bg, 0);
        }
    }
    for &bar in &d.bot_fades {
        unsafe {
            lv_obj_set_style_bg_color(bar, bg, 0);
        }
    }
}

pub fn create(parent: &mut Wdg) {
    let pw = crate::ui::config::PRODUCTION_UI_SIZE as i32;
    let ph = crate::ui::config::PRODUCTION_UI_SIZE as i32;
    let parent_raw = parent.raw_mut();

    // ── Title (fixed, outside scroll container) ──────────────────────────
    let mut lbl = Label::new();
    lbl.set_text(cstr!("Settings"));
    align_to(&mut lbl, parent, Align::TopMid, 0, 25);
    lbl.set_parent(parent);
    let _ = lbl.leak();

    let container_top = 55;
    let container_btm = 55;
    let container_h = ph - container_top - container_btm;

    // ── Scrollable container below title ─────────────────────────────────
    let container = unsafe { lv_obj_create(parent_raw) };
    unsafe {
        lv_obj_set_pos(container, 0, container_top);
        lv_obj_set_size(container, pw, container_h);
        lv_obj_set_style_radius(container, 0, 0);
        lv_obj_set_style_border_width(container, 0, 0);
        lv_obj_set_style_bg_opa(container, 0, 0);
        lv_obj_set_style_pad_top(container, 0, 0);
        lv_obj_set_style_pad_bottom(container, 0, 0);
        lv_obj_set_style_pad_left(container, 0, 0);
        lv_obj_set_style_pad_right(container, 0, 0);
        lv_obj_add_flag(container, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        lv_obj_add_flag(container, lv_obj_flag_t_LV_OBJ_FLAG_SCROLL_ELASTIC);
        lv_obj_set_scroll_dir(container, lv_dir_t_LV_DIR_VER);
        lv_obj_set_scrollbar_mode(container, lv_scrollbar_mode_t_LV_SCROLLBAR_MODE_OFF);
    }

    // ── Config rows ──────────────────────────────────────────────────────
    let mut row_y = 30;
    let row_h = 50;
    let text_color = current_palette().alt_text;

    // Collect label handles for re-theming
    let mut label_idx = 0usize;
    let mut label_handles = [core::ptr::null_mut(); 6];

    macro_rules! config_row {
        ($label:expr) => {{
            let mut row = Button::new();
            row.set_size(pw - 100, row_h);
            unsafe {
                lv_obj_set_parent(row.raw_mut(), container);
                lv_obj_set_pos(row.raw_mut(), 50, row_y);
                lv_obj_set_style_radius(row.raw_mut(), LV_RADIUS_CIRCLE as i32, 0);
            }
            let mut rl = Label::new();
            rl.set_text($label);
            rl.set_parent(&mut row);
            unsafe {
                lv_obj_center(rl.raw_mut());
                lv_obj_set_style_text_color(rl.raw_mut(), lv_color_hex(text_color), 0);
            }
            label_handles[label_idx] = rl.raw_mut();
            label_idx += 1;
            let _ = rl.leak();
            let row_raw = row.raw_mut();
            let _ = row.leak();
            row_y += row_h + 5;
            row_raw
        }};
    }

    // ── Display (theme + brightness) ─────────────────────────────────────
    let disp_row = config_row!(cstr!("Display"));
    unsafe {
        lv_obj_add_event_cb(
            disp_row,
            Some(display_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── WiFi SSID ────────────────────────────────────────────────────────
    let ssid_row = config_row!(cstr!("WiFi SSID"));
    unsafe {
        lv_obj_add_event_cb(
            ssid_row,
            Some(ssid_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── WiFi Password ────────────────────────────────────────────────────
    let pwd_row = config_row!(cstr!("WiFi Password"));
    unsafe {
        lv_obj_add_event_cb(
            pwd_row,
            Some(pwd_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── Ascon Key ────────────────────────────────────────────────────────
    let ascon_row = config_row!(cstr!("Crypto Key"));
    unsafe {
        lv_obj_add_event_cb(
            ascon_row,
            Some(ascon_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── GMT Offset ──────────────────────────────────────────────────────
    let gmt_row = config_row!(cstr!("GMT Offset"));
    unsafe {
        lv_obj_add_event_cb(
            gmt_row,
            Some(gmt_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // ── Power Off ───────────────────────────────────────────────────────
    let power_row = config_row!(cstr!("Power Off"));
    unsafe {
        lv_obj_add_event_cb(
            power_row,
            Some(power_click_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            parent_raw as *mut _,
        );
    }

    // Spacer inside container to force overflow for scrolling
    {
        let spacer = unsafe { lv_obj_create(container) };
        unsafe {
            lv_obj_set_pos(spacer, 0, row_y);
            lv_obj_set_size(spacer, 1, 300);
            lv_obj_set_style_bg_opa(spacer, 0, 0);
            lv_obj_set_style_border_width(spacer, 0, 0);
            lv_obj_remove_flag(spacer, lv_obj_flag_t_LV_OBJ_FLAG_CLICKABLE);
        }
        let _ = spacer;
    }

    // ── Fade overlays (top & bottom) using stepped transparency ──────────
    let fade_h = 4;
    let fade_steps = 5;
    let fade_opa_step = 40;
    let bg = current_palette().bg_color;

    let fade_bottom = container_top + container_h;
    let mut top_fades = [core::ptr::null_mut(); 5];
    let mut bot_fades = [core::ptr::null_mut(); 5];

    for i in 0..fade_steps {
        let top_bar = unsafe { lv_obj_create(parent_raw) };
        unsafe {
            lv_obj_set_pos(top_bar, 0, container_top + i * fade_h);
            lv_obj_set_size(top_bar, pw, fade_h);
            lv_obj_set_style_radius(top_bar, 0, 0);
            lv_obj_set_style_border_width(top_bar, 0, 0);
            lv_obj_set_style_bg_color(top_bar, lv_color_hex(bg), 0);
            let opa = ((fade_steps - i) * fade_opa_step).min(255) as u8;
            lv_obj_set_style_bg_opa(top_bar, opa, 0);
            lv_obj_remove_flag(top_bar, lv_obj_flag_t_LV_OBJ_FLAG_CLICKABLE);
            lv_obj_remove_flag(top_bar, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        }
        top_fades[i as usize] = top_bar;

        let bot_bar = unsafe { lv_obj_create(parent_raw) };
        unsafe {
            lv_obj_set_pos(bot_bar, 0, fade_bottom - (i + 1) * fade_h);
            lv_obj_set_size(bot_bar, pw, fade_h);
            lv_obj_set_style_radius(bot_bar, 0, 0);
            lv_obj_set_style_border_width(bot_bar, 0, 0);
            lv_obj_set_style_bg_color(bot_bar, lv_color_hex(bg), 0);
            let opa = ((fade_steps - i) * fade_opa_step).min(255) as u8;
            lv_obj_set_style_bg_opa(bot_bar, opa, 0);
            lv_obj_remove_flag(bot_bar, lv_obj_flag_t_LV_OBJ_FLAG_CLICKABLE);
            lv_obj_remove_flag(bot_bar, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        }
        bot_fades[i as usize] = bot_bar;
    }

    let _ = container; // keep alive

    // Store handles for re-theming
    let handles = alloc::boxed::Box::leak(alloc::boxed::Box::new(SettingsHandles {
        labels: label_handles,
        top_fades,
        bot_fades,
    }));
    SETTINGS_DATA.store(handles as *mut SettingsHandles, Ordering::Relaxed);
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

unsafe extern "C" fn power_click_cb(e: *mut lv_event_t) {
    unsafe {
        let parent = lv_event_get_user_data(e) as *mut lv_obj_t;
        power_settings_overlay(parent);
    }
}

/// Align a widget relative to its parent with offset.
fn align_to(w: &mut Wdg, _parent: &Wdg, align: Align, x_ofs: i32, y_ofs: i32) {
    let a: lv_bevy_ecs::sys::lv_align_t = align.into();
    unsafe {
        lv_bevy_ecs::sys::lv_obj_align(w.raw_mut(), a, x_ofs, y_ofs);
    }
}
