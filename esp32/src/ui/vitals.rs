use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;

use super::theme::{ThemePalette, current_palette};
use crate::services::rendering::task::SCREEN_W;

const CX: i32 = SCREEN_W / 2;
const BAR_W: i32 = 280;
const BAR_H: i32 = 16;

pub struct Handles {
    pub bpm_range_bar: *mut lv_obj_t,
    pub bpm_slider: *mut lv_obj_t,
    pub bpm_label: *mut lv_obj_t,
    pub spo2_bar: *mut lv_obj_t,
    pub spo2_label: *mut lv_obj_t,
    pub temp_label: *mut lv_obj_t,
}

pub fn create(parent: &mut lv_bevy_ecs::widgets::Wdg) -> Handles {
    let p = parent.raw_mut();
    let pal = current_palette();
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);
    let accent = lv_color_hex(pal.accent_color);

    unsafe {
        // ── BPM ─────────────────────────────────────────────
        let bl = lv_label_create(p);
        lv_label_set_text(bl, c"-- BPM".as_ptr());
        lv_obj_set_style_text_color(bl, text, 0);
        lv_obj_set_style_text_align(bl, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(bl, 200);
        lv_obj_set_pos(bl, CX - 100, 50);

        let rb = lv_bar_create(p);
        lv_bar_set_range(rb, 0, 200);
        lv_bar_set_mode(rb, lv_bar_mode_t_LV_BAR_MODE_RANGE);
        lv_obj_set_size(rb, BAR_W, BAR_H);
        lv_obj_set_pos(rb, CX - BAR_W / 2, 90);
        lv_obj_set_style_bg_color(rb, overlay, 0);
        lv_obj_set_style_bg_opa(rb, 51, 0);
        lv_obj_set_style_radius(rb, BAR_H / 2, 0);
        lv_obj_set_style_bg_color(rb, overlay, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_bg_opa(rb, 77, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_radius(rb, BAR_H / 2, lv_part_t_LV_PART_INDICATOR);
        lv_bar_set_start_value(rb, 0, false);
        lv_bar_set_value(rb, 0, false);

        let sl = lv_slider_create(p);
        lv_slider_set_range(sl, 0, 200);
        lv_obj_set_size(sl, BAR_W, BAR_H);
        lv_obj_set_pos(sl, CX - BAR_W / 2, 90);
        lv_obj_set_style_bg_opa(sl, 0, 0);
        lv_obj_set_style_bg_opa(sl, 0, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_border_opa(sl, 0, 0);
        lv_obj_set_style_shadow_opa(sl, 0, 0);
        lv_obj_set_style_bg_color(sl, accent, lv_part_t_LV_PART_KNOB);
        lv_obj_set_style_bg_opa(sl, 255, lv_part_t_LV_PART_KNOB);
        lv_obj_set_style_radius(sl, 7, lv_part_t_LV_PART_KNOB);
        lv_slider_set_value(sl, 0, false);

        // ── SpO₂ ────────────────────────────────────────────
        let s2l = lv_label_create(p);
        lv_label_set_text(s2l, c"SpO2 --%".as_ptr());
        lv_obj_set_style_text_color(s2l, text, 0);
        lv_obj_set_style_text_align(s2l, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(s2l, 200);
        lv_obj_set_pos(s2l, CX - 100, 160);

        let sb = lv_bar_create(p);
        lv_bar_set_range(sb, 0, 100);
        lv_obj_set_size(sb, BAR_W, BAR_H);
        lv_obj_set_pos(sb, CX - BAR_W / 2, 200);
        lv_obj_set_style_bg_color(sb, overlay, 0);
        lv_obj_set_style_bg_opa(sb, 51, 0);
        lv_obj_set_style_radius(sb, BAR_H / 2, 0);
        lv_obj_set_style_bg_color(
            sb,
            lv_color_hex(pal.healthy_color),
            lv_part_t_LV_PART_INDICATOR,
        );
        lv_obj_set_style_radius(sb, BAR_H / 2, lv_part_t_LV_PART_INDICATOR);
        lv_bar_set_value(sb, 0, false);

        // ── Temperature ─────────────────────────────────────
        let tl = lv_label_create(p);
        lv_label_set_text(tl, c"--°C".as_ptr());
        lv_obj_set_style_text_color(tl, text, 0);
        lv_obj_set_style_text_align(tl, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(tl, 200);
        lv_obj_set_pos(tl, CX - 100, 280);

        Handles {
            bpm_range_bar: rb,
            bpm_slider: sl,
            bpm_label: bl,
            spo2_bar: sb,
            spo2_label: s2l,
            temp_label: tl,
        }
    }
}

pub fn apply_theme(h: &Handles, pal: &ThemePalette) {
    let text = lv_color_hex(pal.text_color);
    let accent = lv_color_hex(pal.accent_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        lv_obj_set_style_text_color(h.bpm_label, text, 0);
        lv_obj_set_style_bg_color(h.bpm_range_bar, overlay, 0);
        lv_obj_set_style_bg_color(h.bpm_range_bar, overlay, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_bg_opa(h.bpm_range_bar, 77, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_bg_color(h.bpm_slider, accent, lv_part_t_LV_PART_KNOB);
        lv_obj_set_style_text_color(h.spo2_label, text, 0);
        lv_obj_set_style_bg_color(h.spo2_bar, overlay, 0);
        lv_obj_set_style_bg_color(
            h.spo2_bar,
            lv_color_hex(pal.healthy_color),
            lv_part_t_LV_PART_INDICATOR,
        );
        lv_obj_set_style_text_color(h.temp_label, text, 0);
    }
}
