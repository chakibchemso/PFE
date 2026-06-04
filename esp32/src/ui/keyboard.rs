use alloc::boxed::Box;
use core::ffi::CStr;
use core::sync::atomic::{AtomicPtr, Ordering};

use lv_bevy_ecs::cstr;
use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::{Button, Label};

use super::theme::{ThemePalette, current_palette};
use crate::services::ui::task::PENDING_SAVES;
use crate::services::ui::task::PendingSave;

// ── Tileview handle for locking scroll while open ────────────────────────
static TV: AtomicPtr<lv_obj_t> = AtomicPtr::new(core::ptr::null_mut());

pub fn set_tileview_handle(h: *mut lv_obj_t) {
    TV.store(h, Ordering::Relaxed);
}

// ── Open-modal handles for re-theming ───────────────────────────────────
struct KbWidgets {
    panel: *mut lv_obj_t,
    ta: *mut lv_obj_t,
    kb: *mut lv_obj_t,
    save_btn: *mut lv_obj_t,
    close_btn: *mut lv_obj_t,
}
static KB_OPEN: AtomicPtr<KbWidgets> = AtomicPtr::new(core::ptr::null_mut());

fn apply_theme(w: &KbWidgets, pal: &ThemePalette) {
    let bg = lv_color_hex(pal.bg_color);
    let text = lv_color_hex(pal.text_color);
    let surface = lv_color_hex(pal.surface_color);
    let overlay = lv_color_hex(pal.overlay_color);
    let accent = lv_color_hex(pal.accent_color);
    unsafe {
        lv_obj_set_style_bg_color(w.panel, bg, 0);
        lv_obj_set_style_bg_opa(w.panel, 255, 0);
        lv_obj_set_style_text_color(w.panel, text, 0);

        lv_obj_set_style_bg_color(w.ta, overlay, 0);
        lv_obj_set_style_text_color(w.ta, text, 0);

        lv_obj_set_style_bg_color(w.kb, bg, 0);
        let items = lv_part_t_LV_PART_ITEMS;
        lv_obj_set_style_bg_color(w.kb, surface, items);
        lv_obj_set_style_bg_color(w.kb, overlay, items | lv_state_t_LV_STATE_CHECKED);
        lv_obj_set_style_bg_color(w.kb, accent, items | lv_state_t_LV_STATE_PRESSED);
        lv_obj_set_style_text_color(w.kb, text, items);
        lv_obj_set_style_text_color(w.kb, text, items | lv_state_t_LV_STATE_CHECKED);

        lv_obj_set_style_bg_color(w.save_btn, accent, 0);
        lv_obj_set_style_text_color(w.save_btn, text, 0);
        lv_obj_set_style_radius(w.save_btn, LV_RADIUS_CIRCLE as i32, 0);

        lv_obj_set_style_bg_color(w.close_btn, overlay, 0);
        lv_obj_set_style_text_color(w.close_btn, text, 0);
        lv_obj_set_style_radius(w.close_btn, LV_RADIUS_CIRCLE as i32, 0);
    }
}

pub fn re_theme() {
    let ptr = KB_OPEN.load(Ordering::Relaxed);
    if !ptr.is_null() {
        apply_theme(unsafe { &*ptr }, current_palette());
    }
}

// ── Modal panel (fullscreen) ─────────────────────────────────────────────
const MODAL_RADIUS: i32 = 0;

// ── Title label ───────────────────────────────────────────────────────────
const TITLE_Y: i32 = 8;

// ── Text entry field ──────────────────────────────────────────────────────
const TA_X: i32 = 95;
const TA_Y: i32 = 40;
const TA_W: i32 = 240;
const TA_H: i32 = 40;

// ── Keyboard ──────────────────────────────────────────────────────────────
const KB_X: i32 = 0;
const KB_Y: i32 = -90;
const KB_W: i32 = 440;
const KB_H: i32 = 240;

// ── Save button ───────────────────────────────────────────────────────────
const BTN_W: i32 = 140;
const BTN_H: i32 = 36;
const BTN_Y: i32 = 345;
const BTN_X: i32 = 140;

// ── Close button (below save) ────────────────────────────────────────────
const CLOSE_BTN_W: i32 = BTN_W;
const CLOSE_BTN_H: i32 = BTN_H;
const CLOSE_BTN_Y: i32 = BTN_Y + BTN_H + 10;
const CLOSE_BTN_X: i32 = BTN_X;

struct ReadyData {
    overlay: *mut lv_obj_t,
    save_key: &'static str,
}

pub unsafe fn lock_tileview() {
    unsafe {
        let tv = TV.load(Ordering::Relaxed);
        if !tv.is_null() {
            lv_obj_remove_flag(tv, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        }
    }
}

pub unsafe fn unlock_tileview() {
    unsafe {
        let tv = TV.load(Ordering::Relaxed);
        if !tv.is_null() {
            lv_obj_add_flag(tv, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        }
    }
}

pub fn keyboard_overlay(
    parent: *mut lv_obj_t,
    title: &CStr,
    current_value: &str,
    placeholder: &CStr,
    is_password: bool,
    max_length: u32,
    save_key: &'static str,
) {
    let pal = current_palette();
    let pw = unsafe { lv_obj_get_width(parent) };
    let ph = unsafe { lv_obj_get_height(parent) };

    // Lock tileview scrolling
    unsafe {
        let tv = TV.load(Ordering::Relaxed);
        if !tv.is_null() {
            lv_obj_remove_flag(tv, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);
        }
    }

    // ── Fullscreen modal panel (no backdrop) ─────────────────────────────
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

    // ── Title ────────────────────────────────────────────────────────────
    let mut title_lbl = Label::new();
    title_lbl.set_text(title);
    unsafe {
        lv_obj_set_parent(title_lbl.raw_mut(), panel);
        lv_obj_align(title_lbl.raw_mut(), lv_align_t_LV_ALIGN_TOP_MID, 0, TITLE_Y);
    }
    let _ = title_lbl.leak();

    // ── Textarea ─────────────────────────────────────────────────────────
    let ta = unsafe { lv_textarea_create(panel) };
    unsafe {
        lv_obj_set_pos(ta, TA_X, TA_Y);
        lv_obj_set_size(ta, TA_W, TA_H);
        lv_textarea_set_one_line(ta, true);
        lv_textarea_set_max_length(ta, max_length);
        lv_textarea_set_placeholder_text(ta, placeholder.as_ptr());
    }

    if is_password {
        unsafe { lv_textarea_set_password_mode(ta, true) };
    }

    if !current_value.is_empty() {
        let bytes = current_value.as_bytes();
        let len = bytes.len().min((max_length as usize).saturating_sub(1));
        let mut buf = [0u8; 256];
        buf[..len].copy_from_slice(&bytes[..len]);
        buf[len] = 0;
        unsafe { lv_textarea_set_text(ta, buf.as_ptr() as *const _) };
    }

    // ── Keyboard ─────────────────────────────────────────────────────────
    let _kb = unsafe { lv_keyboard_create(panel) };
    unsafe {
        lv_obj_set_pos(_kb, KB_X, KB_Y);
        lv_obj_set_size(_kb, KB_W, KB_H);
        lv_keyboard_set_textarea(_kb, ta);
        lv_obj_set_style_radius(_kb, 4, 0);
        lv_keyboard_set_popovers(_kb, true);
    }

    // ── Save button ──────────────────────────────────────────────────────
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

    let raw_ta = ta;

    unsafe {
        lv_obj_add_event_cb(
            save_raw,
            Some(lv_kb_save_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            Box::leak(Box::new(SaveData {
                ta: raw_ta,
                overlay: panel,
                key: save_key,
            })) as *mut SaveData as *mut _,
        );
    }

    // ── Close button (below save) ────────────────────────────────────────
    let mut close_btn = Button::new();
    unsafe {
        lv_obj_set_parent(close_btn.raw_mut(), panel);
        lv_obj_set_pos(close_btn.raw_mut(), CLOSE_BTN_X, CLOSE_BTN_Y);
        lv_obj_set_size(close_btn.raw_mut(), CLOSE_BTN_W, CLOSE_BTN_H);
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

    unsafe {
        lv_obj_add_event_cb(
            close_raw,
            Some(modal_close_cb),
            lv_event_code_t_LV_EVENT_CLICKED,
            panel as *mut _,
        );
    }

    // ── Enter key on textarea ────────────────────────────────────────────
    let ready = Box::leak(Box::new(ReadyData {
        overlay: panel,
        save_key,
    }));
    unsafe {
        lv_obj_add_event_cb(
            ta,
            Some(lv_textarea_ready_cb),
            lv_event_code_t_LV_EVENT_READY,
            ready as *mut ReadyData as *mut _,
        );
    }

    // ── Store handles for re-theming and apply colours ───────────────────
    let w = Box::leak(Box::new(KbWidgets {
        panel,
        ta,
        kb: _kb,
        save_btn: save_raw,
        close_btn: close_raw,
    }));
    KB_OPEN.store(w as *mut KbWidgets, Ordering::Relaxed);
    apply_theme(w, pal);
}

unsafe extern "C" fn panel_cb(e: *mut lv_event_t) {
    unsafe {
        lv_event_stop_bubbling(e);
    }
}

unsafe extern "C" fn modal_close_cb(e: *mut lv_event_t) {
    unsafe {
        let overlay = lv_event_get_user_data(e) as *mut lv_obj_t;
        KB_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete(overlay);
        unlock_tileview();
    }
}

struct SaveData {
    ta: *mut lv_obj_t,
    overlay: *mut lv_obj_t,
    key: &'static str,
}

unsafe extern "C" fn lv_kb_save_cb(e: *mut lv_event_t) {
    unsafe {
        let data = lv_event_get_user_data(e) as *const SaveData;
        let text_ptr = lv_textarea_get_text((*data).ta);
        let text = CStr::from_ptr(text_ptr);
        let data_vec = text.to_bytes().to_vec();
        let _ = PENDING_SAVES.try_send(PendingSave {
            key: (*data).key,
            data: data_vec,
        });
        KB_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}

unsafe extern "C" fn lv_textarea_ready_cb(e: *mut lv_event_t) {
    unsafe {
        let obj = lv_event_get_current_target(e) as *const lv_obj_t;
        let data = lv_event_get_user_data(e) as *const ReadyData;
        let text_ptr = lv_textarea_get_text(obj);
        let text = CStr::from_ptr(text_ptr);
        let data_vec = text.to_bytes().to_vec();
        let _ = PENDING_SAVES.try_send(PendingSave {
            key: (*data).save_key,
            data: data_vec,
        });
        KB_OPEN.store(core::ptr::null_mut(), Ordering::Relaxed);
        lv_obj_delete((*data).overlay);
        unlock_tileview();
    }
}
