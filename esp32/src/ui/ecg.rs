use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;

use super::geom::{CX, scale};
use super::theme::{ThemePalette, current_palette};

// ── Layout constants ─────────────────────────────────────────────────────

const TITLE_Y: i32 = scale(30);
const HR_LBL_Y: i32 = scale(80);
const HR_VALUE_Y: i32 = scale(110);
const LEAD_STATUS_Y: i32 = scale(160);
const CHART_Y: i32 = scale(200);
const CHART_H: i32 = scale(150);
const LABEL_W: i32 = scale(200);
const LABEL_HALF: i32 = LABEL_W / 2;
const BAR_W: i32 = scale(280);
const BAR_HALF: i32 = BAR_W / 2;

pub struct Handles {
    pub title: *mut lv_obj_t,
    pub hr_label: *mut lv_obj_t,
    pub hr_bpm_label: *mut lv_obj_t,
    pub lead_label: *mut lv_obj_t,
    pub chart: *mut lv_obj_t,
    pub chart_mask: *mut lv_obj_t,
    pub ecg_series: *mut lv_chart_series_t,
}

pub fn create(parent: &mut lv_bevy_ecs::widgets::Wdg) -> Handles {
    let p = parent.raw_mut();
    let pal = current_palette();
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        // ── Title ────────────────────────────────────────────
        let title = lv_label_create(p);
        lv_label_set_text(title, c"ECG".as_ptr());
        lv_obj_set_style_text_color(title, text, 0);
        lv_obj_set_style_text_align(title, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(title, LABEL_W);
        lv_obj_set_pos(title, CX - LABEL_HALF, TITLE_Y);

        // ── Large heart rate display ─────────────────────────
        let hr_lbl = lv_label_create(p);
        lv_label_set_text(hr_lbl, c"--".as_ptr());
        lv_obj_set_style_text_color(hr_lbl, text, 0);
        lv_obj_set_style_text_align(hr_lbl, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(hr_lbl, LABEL_W);
        lv_obj_set_pos(hr_lbl, CX - LABEL_HALF, HR_LBL_Y);

        let hr_bpm_lbl = lv_label_create(p);
        lv_label_set_text(hr_bpm_lbl, c"BPM".as_ptr());
        lv_obj_set_style_text_color(hr_bpm_lbl, text, 0);
        lv_obj_set_style_text_align(hr_bpm_lbl, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(hr_bpm_lbl, LABEL_W);
        lv_obj_set_pos(hr_bpm_lbl, CX - LABEL_HALF, HR_VALUE_Y);

        // ── Lead status ──────────────────────────────────────
        let ll = lv_label_create(p);
        lv_label_set_text(ll, c"Lead Off".as_ptr());
        lv_obj_set_style_text_color(ll, text, 0);
        lv_obj_set_style_text_align(ll, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(ll, LABEL_W);
        lv_obj_set_pos(ll, CX - LABEL_HALF, LEAD_STATUS_Y);

        // ── ECG Waveform Chart (mask container + borderless chart) ──
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
        lv_chart_set_point_count(chart, 100);
        lv_chart_set_axis_range(chart, lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y, -1000, 1000);
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

        let ecg_series = lv_chart_add_series(
            chart,
            lv_color_hex(0x90EE90),
            lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y,
        );

        Handles {
            title,
            hr_label: hr_lbl,
            hr_bpm_label: hr_bpm_lbl,
            lead_label: ll,
            chart,
            chart_mask,
            ecg_series,
        }
    }
}

pub fn apply_theme(h: &Handles, pal: &ThemePalette) {
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        lv_obj_set_style_text_color(h.title, text, 0);
        lv_obj_set_style_text_color(h.hr_label, text, 0);
        lv_obj_set_style_text_color(h.hr_bpm_label, text, 0);
        lv_obj_set_style_text_color(h.lead_label, text, 0);
        lv_obj_set_style_border_color(h.chart_mask, overlay, 0);
    }
}
