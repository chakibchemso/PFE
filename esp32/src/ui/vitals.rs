use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;

use super::theme::{ThemePalette, current_palette};
use crate::services::rendering::task::SCREEN_W;

// ── Layout constants ─────────────────────────────────────────────────────

const CX: i32 = SCREEN_W / 2;
const TITLE_Y: i32 = 30;
const LABEL_W: i32 = 200;
const LABEL_HALF: i32 = LABEL_W / 2;
const BAR_W: i32 = 280;
const BAR_H: i32 = 16;
const BAR_HALF: i32 = BAR_W / 2;

const BPM_LBL_Y: i32 = 80;
const BPM_BAR_Y: i32 = 110;
const SPO2_LBL_Y: i32 = 150;
const SPO2_BAR_Y: i32 = 180;
const TEMP_LBL_Y: i32 = 220;
const CHART_Y: i32 = 250;
const CHART_H: i32 = 130;

pub struct Handles {
    pub title: *mut lv_obj_t,
    pub bpm_range_bar: *mut lv_obj_t,
    pub bpm_slider: *mut lv_obj_t,
    pub bpm_label: *mut lv_obj_t,
    pub spo2_bar: *mut lv_obj_t,
    pub spo2_label: *mut lv_obj_t,
    pub temp_label: *mut lv_obj_t,
    pub chart: *mut lv_obj_t,
    pub chart_mask: *mut lv_obj_t,
    pub red_series: *mut lv_chart_series_t,
    pub ir_series: *mut lv_chart_series_t,
}

pub fn create(parent: &mut lv_bevy_ecs::widgets::Wdg) -> Handles {
    let p = parent.raw_mut();
    let pal = current_palette();
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);
    let accent = lv_color_hex(pal.accent_color);

    unsafe {
        // ── Title ────────────────────────────────────────────
        let title = lv_label_create(p);
        lv_label_set_text(title, c"Vitals".as_ptr());
        lv_obj_set_style_text_color(title, text, 0);
        lv_obj_set_style_text_align(title, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(title, LABEL_W);
        lv_obj_set_pos(title, CX - LABEL_HALF, TITLE_Y);

        // ── BPM ──────────────────────────────────────────────
        let bl = lv_label_create(p);
        lv_label_set_text(bl, c"-- BPM".as_ptr());
        lv_obj_set_style_text_color(bl, text, 0);
        lv_obj_set_style_text_align(bl, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(bl, LABEL_W);
        lv_obj_set_pos(bl, CX - LABEL_HALF, BPM_LBL_Y);

        let rb = lv_bar_create(p);
        lv_bar_set_range(rb, 0, 200);
        lv_bar_set_mode(rb, lv_bar_mode_t_LV_BAR_MODE_RANGE);
        lv_obj_set_size(rb, BAR_W, BAR_H);
        lv_obj_set_pos(rb, CX - BAR_HALF, BPM_BAR_Y);
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
        lv_obj_set_pos(sl, CX - BAR_HALF, BPM_BAR_Y);
        lv_obj_set_style_bg_opa(sl, 0, 0);
        lv_obj_set_style_bg_opa(sl, 0, lv_part_t_LV_PART_INDICATOR);
        lv_obj_set_style_border_opa(sl, 0, 0);
        lv_obj_set_style_shadow_opa(sl, 0, 0);
        lv_obj_set_style_bg_color(sl, accent, lv_part_t_LV_PART_KNOB);
        lv_obj_set_style_bg_opa(sl, 255, lv_part_t_LV_PART_KNOB);
        lv_obj_set_style_radius(sl, 7, lv_part_t_LV_PART_KNOB);
        lv_slider_set_value(sl, 10, false);
        lv_obj_remove_flag(sl, lv_obj_flag_t_LV_OBJ_FLAG_CLICKABLE);
        lv_obj_remove_flag(sl, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);

        // ── SpO₂ ─────────────────────────────────────────────
        let s2l = lv_label_create(p);
        lv_label_set_text(s2l, c"SpO2 --%".as_ptr());
        lv_obj_set_style_text_color(s2l, text, 0);
        lv_obj_set_style_text_align(s2l, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(s2l, LABEL_W);
        lv_obj_set_pos(s2l, CX - LABEL_HALF, SPO2_LBL_Y);

        let sb = lv_bar_create(p);
        lv_bar_set_range(sb, 0, 100);
        lv_obj_set_size(sb, BAR_W, BAR_H);
        lv_obj_set_pos(sb, CX - BAR_HALF, SPO2_BAR_Y);
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

        // ── Temperature ──────────────────────────────────────
        let tl = lv_label_create(p);
        lv_label_set_text(tl, c"--°C".as_ptr());
        lv_obj_set_style_text_color(tl, text, 0);
        lv_obj_set_style_text_align(tl, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(tl, LABEL_W);
        lv_obj_set_pos(tl, CX - LABEL_HALF, TEMP_LBL_Y);

        // ── PPG Waveform Chart (mask container + borderless chart) ──
        let chart_mask = lv_obj_create(p);
        lv_obj_set_size(chart_mask, BAR_W, CHART_H);
        lv_obj_set_pos(chart_mask, CX - BAR_HALF, CHART_Y);
        lv_obj_set_style_radius(chart_mask, 12, 0);
        lv_obj_set_style_clip_corner(chart_mask, true, 0);
        lv_obj_set_style_border_width(chart_mask, 2, 0);
        lv_obj_set_style_border_color(chart_mask, overlay, 0);
        lv_obj_set_style_bg_opa(chart_mask, 0, 0);
        lv_obj_set_style_pad_top(chart_mask, 0, 0);
        lv_obj_set_style_pad_bottom(chart_mask, 0, 0);
        lv_obj_set_style_pad_left(chart_mask, 0, 0);
        lv_obj_set_style_pad_right(chart_mask, 0, 0);
        lv_obj_remove_flag(chart_mask, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);

        let chart = lv_chart_create(chart_mask);
        lv_chart_set_type(chart, lv_chart_type_t_LV_CHART_TYPE_LINE);
        lv_chart_set_update_mode(chart, lv_chart_update_mode_t_LV_CHART_UPDATE_MODE_CIRCULAR);
        lv_chart_set_point_count(chart, 200);
        lv_chart_set_axis_range(chart, lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y, -500, 500);
        lv_obj_set_size(chart, BAR_W, CHART_H);
        lv_obj_set_pos(chart, -1, -1);
        lv_obj_set_style_border_width(chart, 0, 0);
        lv_obj_set_style_bg_opa(chart, 0, 0);
        lv_obj_set_style_radius(chart, 0, 0);
        lv_obj_set_style_pad_top(chart, 0, 0);
        lv_obj_set_style_pad_bottom(chart, 0, 0);
        lv_obj_set_style_pad_left(chart, 0, 0);
        lv_obj_set_style_pad_right(chart, 0, 0);
        lv_obj_remove_flag(chart, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);

        let red_series = lv_chart_add_series(
            chart,
            lv_color_hex(0xea999c),
            lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y,
        );
        let ir_series = lv_chart_add_series(
            chart,
            lv_color_hex(0xf4d59b),
            lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y,
        );

        Handles {
            title,
            bpm_range_bar: rb,
            bpm_slider: sl,
            bpm_label: bl,
            spo2_bar: sb,
            spo2_label: s2l,
            temp_label: tl,
            chart,
            chart_mask,
            red_series,
            ir_series,
        }
    }
}

pub fn apply_theme(h: &Handles, pal: &ThemePalette) {
    let text = lv_color_hex(pal.text_color);
    let accent = lv_color_hex(pal.accent_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        lv_obj_set_style_text_color(h.title, text, 0);
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
        lv_obj_set_style_border_color(h.chart_mask, overlay, 0);
    }
}
