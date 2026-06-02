use libm::{cosf, sinf};

use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::Wdg;

use super::theme::{ThemePalette, current_palette};
use crate::services::rendering::task::{SCREEN_H, SCREEN_W};

const CX: i32 = SCREEN_W / 2;
const CY: i32 = SCREEN_H / 2;
const DOT_R: i32 = 180;
const DEG2RAD: f32 = core::f32::consts::PI / 180.0;

#[derive(Clone, Copy)]
pub struct Handles {
    pub hour_hand: *mut lv_obj_t,
    pub minute_hand: *mut lv_obj_t,
    pub second_hand: *mut lv_obj_t,
    pub center_dot: *mut lv_obj_t,
    pub digital_time: *mut lv_obj_t,
    pub wifi_led: *mut lv_obj_t,
    pub mqtt_led: *mut lv_obj_t,
    pub wifi_label: *mut lv_obj_t,
    pub mqtt_label: *mut lv_obj_t,
    pub battery_bar: *mut lv_obj_t,
    pub battery_pct: *mut lv_obj_t,
}

fn deg_to_rad(d: f32) -> f32 {
    d * DEG2RAD
}

pub fn create(parent: &mut Wdg) -> Handles {
    let p = parent.raw_mut();
    let pal = current_palette();
    let text = lv_color_hex(pal.text_color);
    let accent = lv_color_hex(pal.accent_color);
    let overlay = lv_color_hex(pal.overlay_color);

    // Safety: all LVGL FFI calls operate on valid widget pointers and run
    // on the same core as the LVGL timer handler.
    unsafe {
        for i in 0..12 {
            let a = deg_to_rad(i as f32 * 30.0);
            let dx = (DOT_R as f32 * sinf(a)) as i32;
            let dy = (DOT_R as f32 * cosf(a)) as i32;
            let t = lv_obj_create(p);
            lv_obj_set_size(t, 8, 8);
            lv_obj_set_style_bg_color(t, accent, 0);
            lv_obj_set_style_bg_opa(t, 255, 0);
            lv_obj_set_style_radius(t, 4, 0);
            lv_obj_set_pos(t, CX + dx - 4, CY - dy - 4);
        }
        for i in 0..60 {
            if i % 5 == 0 {
                continue;
            }
            let a = deg_to_rad(i as f32 * 6.0);
            let dx = (DOT_R as f32 * sinf(a)) as i32;
            let dy = (DOT_R as f32 * cosf(a)) as i32;
            let t = lv_obj_create(p);
            lv_obj_set_size(t, 4, 4);
            lv_obj_set_style_bg_color(t, overlay, 0);
            lv_obj_set_style_bg_opa(t, 255, 0);
            lv_obj_set_style_radius(t, 2, 0);
            lv_obj_set_pos(t, CX + dx - 2, CY - dy - 2);
        }

        let hh = lv_obj_create(p);
        lv_obj_set_size(hh, 6, 50);
        lv_obj_set_style_bg_color(hh, text, 0);
        lv_obj_set_style_bg_opa(hh, 255, 0);
        lv_obj_set_style_radius(hh, 3, 0);
        lv_obj_set_pos(hh, CX - 3, CY - 50);
        lv_obj_set_style_transform_pivot_x(hh, 3, 0);
        lv_obj_set_style_transform_pivot_y(hh, 50, 0);

        let mh = lv_obj_create(p);
        lv_obj_set_size(mh, 4, 80);
        lv_obj_set_style_bg_color(mh, text, 0);
        lv_obj_set_style_bg_opa(mh, 255, 0);
        lv_obj_set_style_radius(mh, 2, 0);
        lv_obj_set_pos(mh, CX - 2, CY - 80);
        lv_obj_set_style_transform_pivot_x(mh, 2, 0);
        lv_obj_set_style_transform_pivot_y(mh, 80, 0);

        let sh = lv_obj_create(p);
        lv_obj_set_size(sh, 2, 100);
        lv_obj_set_style_bg_color(sh, accent, 0);
        lv_obj_set_style_bg_opa(sh, 255, 0);
        lv_obj_set_style_radius(sh, 1, 0);
        lv_obj_set_pos(sh, CX - 1, CY - 100);
        lv_obj_set_style_transform_pivot_x(sh, 1, 0);
        lv_obj_set_style_transform_pivot_y(sh, 100, 0);

        let cd = lv_obj_create(p);
        lv_obj_set_size(cd, 12, 12);
        lv_obj_set_style_bg_color(cd, accent, 0);
        lv_obj_set_style_bg_opa(cd, 255, 0);
        lv_obj_set_style_radius(cd, 6, 0);
        lv_obj_set_pos(cd, CX - 6, CY - 6);

        let dt = lv_label_create(p);
        lv_label_set_text(dt, c"00:00:00".as_ptr());
        lv_obj_set_style_text_color(dt, text, 0);
        lv_obj_set_style_text_align(dt, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(dt, 120);
        lv_obj_set_pos(dt, CX - 60, CY + 100);

        // ── WiFi indicator ─────────────────────────────────────
        let wl = lv_led_create(p);
        lv_obj_set_pos(wl, CX - 75, CY + 140);
        lv_obj_set_size(wl, 10, 10);
        lv_led_set_color(wl, lv_color_hex(0xa6e3a1));
        lv_led_off(wl);

        let wlb = lv_label_create(p);
        lv_label_set_text(wlb, c"WiFi".as_ptr());
        lv_obj_set_style_text_color(wlb, overlay, 0);
        lv_obj_align_to(
            wlb,
            wl,
            lv_align_t_LV_ALIGN_OUT_RIGHT_MID,
            6,
            0,
        );

        // ── MQTT indicator ─────────────────────────────────────
        let ml = lv_led_create(p);
        lv_obj_set_pos(ml, CX - 10, CY + 140);
        lv_obj_set_size(ml, 10, 10);
        lv_led_set_color(ml, lv_color_hex(0xa6e3a1));
        lv_led_off(ml);

        let mlb = lv_label_create(p);
        lv_label_set_text(mlb, c"MQTT".as_ptr());
        lv_obj_set_style_text_color(mlb, overlay, 0);
        lv_obj_align_to(
            mlb,
            ml,
            lv_align_t_LV_ALIGN_OUT_RIGHT_MID,
            6,
            0,
        );

        // ── Battery ────────────────────────────────────────────
        let bb = lv_bar_create(p);
        lv_obj_set_size(bb, 140, 14);
        lv_obj_set_pos(bb, CX - 70, CY + 170);
        lv_obj_set_style_bg_color(bb, overlay, 0);
        lv_obj_set_style_bg_opa(bb, 51, 0);
        lv_obj_set_style_radius(bb, 7, 0);
        lv_obj_set_style_bg_color(bb, accent, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_radius(bb, 7, lv_part_t_LV_PART_INDICATOR);
        lv_bar_set_value(bb, 85, false);

        let bp = lv_label_create(p);
        lv_label_set_text(bp, c"85%".as_ptr());
        lv_obj_set_style_text_color(bp, overlay, 0);
        lv_obj_align_to(
            bp,
            bb,
            lv_align_t_LV_ALIGN_OUT_RIGHT_MID,
            8,
            0,
        );

        Handles {
            hour_hand: hh,
            minute_hand: mh,
            second_hand: sh,
            center_dot: cd,
            digital_time: dt,
            wifi_led: wl,
            mqtt_led: ml,
            wifi_label: wlb,
            mqtt_label: mlb,
            battery_bar: bb,
            battery_pct: bp,
        }
    }
}

pub fn apply_theme(h: &Handles, pal: &ThemePalette) {
    let text = lv_color_hex(pal.text_color);
    let accent = lv_color_hex(pal.accent_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        lv_obj_set_style_bg_color(h.hour_hand, text, 0);
        lv_obj_set_style_bg_color(h.minute_hand, text, 0);
        lv_obj_set_style_bg_color(h.second_hand, accent, 0);
        lv_obj_set_style_bg_color(h.center_dot, accent, 0);

        lv_obj_set_style_text_color(h.digital_time, text, 0);
        lv_obj_set_style_text_color(h.wifi_label, text, 0);
        lv_obj_set_style_text_color(h.mqtt_label, text, 0);

        lv_obj_set_style_bg_color(h.battery_bar, overlay, 0);
        lv_obj_set_style_bg_color(h.battery_bar, accent, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_text_color(h.battery_pct, text, 0);
    }
}
