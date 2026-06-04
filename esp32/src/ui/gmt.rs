use core::ffi::CStr;
use core::sync::atomic::{AtomicI8, AtomicPtr, Ordering};

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::{Button, Label};

use super::keyboard::{lock_tileview, unlock_tileview};
use super::theme::{ThemePalette, current_palette};
use crate::services::ui::task::PENDING_SAVES;
use crate::services::ui::task::PendingSave;

// ── Open-modal handles for re-theming ───────────────────────────────────
struct GmtData {
    overlay: *mut lv_obj_t,
    val_label: *mut lv_obj_t,
    dec_btn: *mut lv_obj_t,
    inc_btn: *mut lv_obj_t,
    save_btn: *mut lv_obj_t,
    close_btn: *mut lv_obj_t,
    offset: AtomicI8,
}
static GMT_OPEN: AtomicPtr<GmtData> = AtomicPtr::new(core::ptr::null_mut());

fn apply_theme(d: &GmtData, pal: &ThemePalette) {
    let bg = lv_color_hex(pal.bg_color);
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);
    let accent = lv_color_hex(pal.accent_color);
    unsafe {
        lv_obj_set_style_bg_color(d.overlay, bg, 0);
        lv_obj_set_style_bg_opa(d.overlay, 255, 0);
        lv_obj_set_style_text_color(d.overlay, text, 0);

        lv_obj_set_style_text_color(d.val_label, text, 0);

        lv_obj_set_style_bg_color(d.dec_btn, accent, 0);
        lv_obj_set_style_text_color(d.dec_btn, text, 0);
        lv_obj_set_style_radius(d.dec_btn, LV_RADIUS_CIRCLE as i32, 0);

        lv_obj_set_style_bg_color(d.inc_btn, accent, 0);
        lv_obj_set_style_text_color(d.inc_btn, text, 0);
        lv_obj_set_style_radius(d.inc_btn, LV_RADIUS_CIRCLE as i32, 0);

        lv_obj_set_style_bg_color(d.save_btn, accent, 0);
        lv_obj_set_style_text_color(d.save_btn, text, 0);
        lv_obj_set_style_radius(d.save_btn, LV_RADIUS_CIRCLE as i32, 0);

        lv_obj_set_style_bg_color(d.close_btn, overlay, 0);
        lv_obj_set_style_text_color(d.close_btn, text, 0);
        lv_obj_set_style_radius(d.close_btn, LV_RADIUS_CIRCLE as i32, 0);
    }
}

pub fn re_theme() {
    let ptr = GMT_OPEN.load(Ordering::Relaxed);
    if !ptr.is_null() {
        apply_theme(unsafe { &*ptr }, current_palette());
    }
}

// ── Modal panel (fullscreen) ────────────────────────────────────────
const MODAL_RADIUS: i32 = 0;

// ── Title label ──────────────────────────────────────────────────────
const TITLE_Y: i32 = 10;

// ── Step buttons (− / +) ─────────────────────────────────────────────
const STEP_BTN_SIZE: i32 = 64;
const STEP_BTN_X_OFF: i32 = 120;

// ── Save / Close buttons ─────────────────────────────────────────────
const BTN_W: i32 = 140;
const BTN_H: i32 = 36;
const BTN_Y: i32 = 320;
const BTN_X: i32 = 140;
const CLOSE_BTN_Y: i32 = BTN_Y + BTN_H + 10;

// ── Value label Y offset from center ─────────────────────────────────
const VAL_Y_OFF: i32 = 0;

const GMT_MIN: i8 = -12;
const GMT_MAX: i8 = 14;

pub static LAST_GMT_OFFSET: AtomicI8 = AtomicI8::new(0);

fn fmt_gmt(buf: &mut [u8; 16], offset: i8) {
    let sign = if offset >= 0 { b'+' } else { b'-' };
    let abs = offset.unsigned_abs();
    buf[0] = b'G';
    buf[1] = b'M';
    buf[2] = b'T';
    buf[3] = sign;
    if abs >= 10 {
        buf[4] = b'0' + (abs / 10) as u8;
        buf[5] = b'0' + (abs % 10) as u8;
        buf[6] = 0;
    } else {
        buf[4] = b'0' + abs as u8;
        buf[5] = 0;
    }
}

unsafe fn update_label(label: *mut lv_obj_t, offset: i8) {
    unsafe {
        let mut buf = [0u8; 16];
        fmt_gmt(&mut buf, offset);
        lv_label_set_text(label, buf.as_ptr() as *const _);
    }
}

unsafe extern "C" fn panel_cb(e: *mut lv_event_t) {
    unsafe {
        lv_event_stop_bubbling(e);
    }
}

unsafe extern "C" fn modal_close_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *const GmtData;
        GMT_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}

unsafe extern "C" fn gmt_dec_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *mut GmtData;
        let old = (*data).offset.load(Ordering::Relaxed);
        let new = (old - 1).max(GMT_MIN);
        if new != old {
            (*data).offset.store(new, Ordering::Relaxed);
            update_label((*data).val_label, new);
        }
    }
}

unsafe extern "C" fn gmt_inc_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *mut GmtData;
        let old = (*data).offset.load(Ordering::Relaxed);
        let new = (old + 1).min(GMT_MAX);
        if new != old {
            (*data).offset.store(new, Ordering::Relaxed);
            update_label((*data).val_label, new);
        }
    }
}

unsafe extern "C" fn gmt_save_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *const GmtData;
        let off = (*data).offset.load(Ordering::Relaxed);
        let _ = PENDING_SAVES.try_send(PendingSave {
            key: crate::services::storage::KEY_GMT_OFFSET,
            data: alloc::vec![off as u8],
        });
        GMT_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}

pub fn gmt_overlay(parent: *mut lv_obj_t, initial_offset: i8) {
    let pal = current_palette();
    let pw = unsafe { lv_obj_get_width(parent) };
    let ph = unsafe { lv_obj_get_height(parent) };

    // Lock tileview scrolling
    unsafe { lock_tileview() }

    // ── Fullscreen modal panel (no backdrop) ─────────────────────────
    let panel = unsafe { lv_obj_create(parent) };
    unsafe {
        lv_obj_set_size(panel, pw, ph);
        lv_obj_set_pos(panel, 0, 0);
        lv_obj_remove_flag(panel, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
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
    title_lbl.set_text(cstr!("GMT Offset"));
    unsafe {
        lv_obj_set_parent(title_lbl.raw_mut(), panel);
        lv_obj_align(title_lbl.raw_mut(), lv_align_t_LV_ALIGN_TOP_MID, 0, TITLE_Y);
    }
    let _ = title_lbl.leak();

    // Offset value label (centered, shifted up)
    let mut val_lbl = Label::new();
    unsafe {
        let raw = val_lbl.raw_mut();
        lv_obj_set_parent(raw, panel);
        lv_obj_set_style_text_font(raw, &lv_font_montserrat_32, 0);
        lv_obj_align(raw, lv_align_t_LV_ALIGN_CENTER, 0, VAL_Y_OFF);
        update_label(raw, initial_offset.clamp(GMT_MIN, GMT_MAX));
    }
    let val_raw = val_lbl.raw_mut();
    let _ = val_lbl.leak();

    // "−" button (left of label)
    let mut dec_btn = Button::new();
    unsafe {
        lv_obj_set_parent(dec_btn.raw_mut(), panel);
        lv_obj_set_size(dec_btn.raw_mut(), STEP_BTN_SIZE, STEP_BTN_SIZE);
        lv_obj_align(
            dec_btn.raw_mut(),
            lv_align_t_LV_ALIGN_CENTER,
            -STEP_BTN_X_OFF,
            VAL_Y_OFF,
        );
    }
    let mut dec_lbl = Label::new();
    dec_lbl.set_text(cstr!("-"));
    unsafe {
        lv_obj_set_parent(dec_lbl.raw_mut(), dec_btn.raw_mut());
        lv_obj_center(dec_lbl.raw_mut());
    }
    let _ = dec_lbl.leak();
    let dec_raw = dec_btn.raw_mut();
    let _ = dec_btn.leak();

    // "+" button (right of label)
    let mut inc_btn = Button::new();
    unsafe {
        lv_obj_set_parent(inc_btn.raw_mut(), panel);
        lv_obj_set_size(inc_btn.raw_mut(), STEP_BTN_SIZE, STEP_BTN_SIZE);
        lv_obj_align(
            inc_btn.raw_mut(),
            lv_align_t_LV_ALIGN_CENTER,
            STEP_BTN_X_OFF,
            VAL_Y_OFF,
        );
    }
    let mut inc_lbl = Label::new();
    inc_lbl.set_text(cstr!("+"));
    unsafe {
        lv_obj_set_parent(inc_lbl.raw_mut(), inc_btn.raw_mut());
        lv_obj_center(inc_lbl.raw_mut());
    }
    let _ = inc_lbl.leak();
    let inc_raw = inc_btn.raw_mut();
    let _ = inc_btn.leak();

    // ── Save button ──────────────────────────────────────────────────
    let mut save_btn = Button::new();
    unsafe {
        lv_obj_set_parent(save_btn.raw_mut(), panel);
        lv_obj_set_pos(save_btn.raw_mut(), BTN_X, BTN_Y);
        lv_obj_set_size(save_btn.raw_mut(), BTN_W, BTN_H);
    }
    let mut save_lbl = Label::new();
    save_lbl.set_text(cstr!("Save"));
    unsafe {
        lv_obj_set_parent(save_lbl.raw_mut(), save_btn.raw_mut());
        lv_obj_center(save_lbl.raw_mut());
    }
    let _ = save_lbl.leak();
    let save_raw = save_btn.raw_mut();
    let _ = save_btn.leak();

    // ── Close button (below save) ────────────────────────────────────
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
    let data: &'static mut GmtData = alloc::boxed::Box::leak(alloc::boxed::Box::new(GmtData {
        overlay: panel,
        val_label: val_raw,
        dec_btn: dec_raw,
        inc_btn: inc_raw,
        save_btn: save_raw,
        close_btn: close_raw,
        offset: AtomicI8::new(initial_offset.clamp(GMT_MIN, GMT_MAX)),
    }));

    unsafe {
        lv_obj_add_event_cb(
            dec_raw,
            Some(gmt_dec_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            data as *mut GmtData as *mut _,
        );
        lv_obj_add_event_cb(
            inc_raw,
            Some(gmt_inc_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            data as *mut GmtData as *mut _,
        );
        lv_obj_add_event_cb(
            save_raw,
            Some(gmt_save_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            data as *mut GmtData as *mut _,
        );
        lv_obj_add_event_cb(
            close_raw,
            Some(modal_close_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            data as *mut GmtData as *mut _,
        );
    }

    // ── Apply theme colors ───────────────────────────────────────────
    GMT_OPEN.store(data as *mut GmtData, Ordering::Relaxed);
    apply_theme(data, pal);
}
