use libm::{cosf, sinf};

use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;
use lv_bevy_ecs::widgets::Wdg;

use super::geom::{CX, CY, scale};
use super::theme::{ThemePalette, current_palette};

const DOT_R: i32 = scale(200);
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

    unsafe {
        for i in 0..12 {
            let a = deg_to_rad(i as f32 * 30.0);
            let dx = (DOT_R as f32 * sinf(a)) as i32;
            let dy = (DOT_R as f32 * cosf(a)) as i32;
            let t = lv_obj_create(p);
            lv_obj_set_size(t, scale(10), scale(10));
            lv_obj_set_style_bg_color(t, accent, 0);
            lv_obj_set_style_bg_opa(t, 255, 0);
            lv_obj_set_style_radius(t, scale(5), 0);
            lv_obj_set_pos(t, CX + dx - scale(5), CY - dy - scale(5));
        }
        for i in 0..60 {
            if i % 5 == 0 {
                continue;
            }
            let a = deg_to_rad(i as f32 * 6.0);
            let dx = (DOT_R as f32 * sinf(a)) as i32;
            let dy = (DOT_R as f32 * cosf(a)) as i32;
            let t = lv_obj_create(p);
            lv_obj_set_size(t, scale(5), scale(5));
            lv_obj_set_style_bg_color(t, overlay, 0);
            lv_obj_set_style_bg_opa(t, 255, 0);
            lv_obj_set_style_radius(t, scale(2), 0);
            lv_obj_set_pos(t, CX + dx - scale(2), CY - dy - scale(2));
        }

        let hh = lv_obj_create(p);
        lv_obj_set_size(hh, scale(8), scale(60));
        lv_obj_set_style_bg_color(hh, text, 0);
        lv_obj_set_style_bg_opa(hh, 255, 0);
        lv_obj_set_style_radius(hh, scale(4), 0);
        lv_obj_set_pos(hh, CX - scale(4), CY - scale(60));
        lv_obj_set_style_transform_pivot_x(hh, scale(4), 0);
        lv_obj_set_style_transform_pivot_y(hh, scale(60), 0);

        let mh = lv_obj_create(p);
        lv_obj_set_size(mh, scale(6), scale(95));
        lv_obj_set_style_bg_color(mh, text, 0);
        lv_obj_set_style_bg_opa(mh, 255, 0);
        lv_obj_set_style_radius(mh, scale(3), 0);
        lv_obj_set_pos(mh, CX - scale(3), CY - scale(95));
        lv_obj_set_style_transform_pivot_x(mh, scale(3), 0);
        lv_obj_set_style_transform_pivot_y(mh, scale(95), 0);

        let sh = lv_obj_create(p);
        lv_obj_set_size(sh, scale(3), scale(120));
        lv_obj_set_style_bg_color(sh, accent, 0);
        lv_obj_set_style_bg_opa(sh, 255, 0);
        lv_obj_set_style_radius(sh, scale(1), 0);
        lv_obj_set_pos(sh, CX - scale(1), CY - scale(120));
        lv_obj_set_style_transform_pivot_x(sh, scale(1), 0);
        lv_obj_set_style_transform_pivot_y(sh, scale(120), 0);

        let cd = lv_obj_create(p);
        lv_obj_set_size(cd, scale(16), scale(16));
        lv_obj_set_style_bg_color(cd, accent, 0);
        lv_obj_set_style_bg_opa(cd, 255, 0);
        lv_obj_set_style_radius(cd, scale(8), 0);
        lv_obj_set_pos(cd, CX - scale(8), CY - scale(8));

        let dt = lv_label_create(p);
        lv_label_set_text(dt, c"00:00:00".as_ptr());
        lv_obj_set_style_text_color(dt, text, 0);
        lv_obj_set_style_text_align(dt, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(dt, scale(140));
        lv_obj_set_pos(dt, CX - scale(70), scale(100));

        // ── WiFi indicator ─────────────────────────────────────
        let wl = lv_led_create(p);
        lv_obj_set_pos(wl, CX - scale(65), CY + scale(115));
        lv_obj_set_size(wl, scale(10), scale(10));
        lv_led_set_color(wl, lv_color_hex(0xa6e3a1));
        lv_led_off(wl);

        let wlb = lv_label_create(p);
        lv_label_set_text(wlb, c"WiFi".as_ptr());
        lv_obj_set_style_text_color(wlb, overlay, 0);
        lv_obj_align_to(wlb, wl, lv_align_t_LV_ALIGN_OUT_RIGHT_MID, scale(6), 0);

        // ── MQTT indicator ─────────────────────────────────────
        let ml = lv_led_create(p);
        lv_obj_set_pos(ml, CX - scale(0), CY + scale(115));
        lv_obj_set_size(ml, scale(10), scale(10));
        lv_led_set_color(ml, lv_color_hex(0xa6e3a1));
        lv_led_off(ml);

        let mlb = lv_label_create(p);
        lv_label_set_text(mlb, c"MQTT".as_ptr());
        lv_obj_set_style_text_color(mlb, overlay, 0);
        lv_obj_align_to(mlb, ml, lv_align_t_LV_ALIGN_OUT_RIGHT_MID, scale(6), 0);

        // ── Battery ────────────────────────────────────────────
        let bb = lv_bar_create(p);
        lv_obj_set_size(bb, scale(140), scale(14));
        lv_obj_set_pos(bb, CX - scale(70), CY + scale(140));
        lv_obj_set_style_bg_color(bb, overlay, 0);
        lv_obj_set_style_bg_opa(bb, 51, 0);
        lv_obj_set_style_radius(bb, scale(7), 0);
        lv_obj_set_style_bg_color(bb, accent, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_radius(bb, scale(7), lv_part_t_LV_PART_INDICATOR);
        lv_bar_set_value(bb, 0, false);

        let bp = lv_label_create(p);
        lv_label_set_text(bp, c"NA".as_ptr());
        lv_obj_set_style_text_color(bp, overlay, 0);
        lv_obj_align_to(bp, bb, lv_align_t_LV_ALIGN_OUT_RIGHT_MID, scale(8), 0);

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
