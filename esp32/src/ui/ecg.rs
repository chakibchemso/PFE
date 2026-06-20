//! ECG data display pane — shows a simulated ECG waveform chart with lead
//! status, heart rate, R2R interval, and HRV readouts for investor demos.
//! UI-only — no real ECG data is acquired.

use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;

use super::geom::{CX, scale};
use super::theme::{ThemePalette, current_palette};

const TITLE_Y: i32 = scale(30);
const LABEL_W: i32 = scale(400);
const LABEL_HALF: i32 = LABEL_W / 2;
const BAR_W: i32 = scale(280);
const BAR_HALF: i32 = BAR_W / 2;

pub struct Handles {
    pub title: *mut lv_obj_t,
    pub hr_label: *mut lv_obj_t,
    pub r2r_label: *mut lv_obj_t,
    pub lead_label: *mut lv_obj_t,
    pub chart: *mut lv_obj_t,
    pub chart_mask: *mut lv_obj_t,
    pub ecg_series: *mut lv_chart_series_t,
    pub hrv_label: *mut lv_obj_t,
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

        // ── Heart rate (left) ─────────────────────────────────
        let hr_label = lv_label_create(p);
        lv_label_set_text(hr_label, c"-- BPM".as_ptr());
        lv_obj_set_style_text_color(hr_label, text, 0);
        lv_obj_set_style_text_align(hr_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(hr_label, scale(180));
        lv_obj_set_pos(hr_label, scale(60), scale(75));

        // ── R2R interval (right) ──────────────────────────────
        let r2r_label = lv_label_create(p);
        lv_label_set_text(r2r_label, c"R2R: --- ms".as_ptr());
        lv_obj_set_style_text_color(r2r_label, overlay, 0);
        lv_obj_set_style_text_align(r2r_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(r2r_label, scale(180));
        lv_obj_set_pos(r2r_label, scale(250), scale(75));

        // ── Lead status ───────────────────────────────────────
        let lead_label = lv_label_create(p);
        lv_label_set_text(lead_label, c"Lead I  \u{25CF} Connected".as_ptr());
        lv_obj_set_style_text_color(lead_label, overlay, 0);
        lv_obj_set_style_text_align(lead_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(lead_label, LABEL_W);
        lv_obj_set_pos(lead_label, CX - LABEL_HALF, scale(120));

        // ── ECG Waveform Chart ────────────────────────────────
        let chart_mask = lv_obj_create(p);
        lv_obj_set_size(chart_mask, BAR_W, scale(200));
        lv_obj_set_pos(chart_mask, CX - BAR_HALF, scale(160));
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
        lv_chart_set_axis_range(chart, lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y, -100, 100);
        lv_obj_set_size(chart, BAR_W, scale(200));
        lv_obj_set_pos(chart, -1, -1);
        lv_obj_set_style_border_width(chart, 0, 0);
        lv_obj_set_style_bg_opa(chart, 0, 0);
        lv_obj_set_style_radius(chart, 0, 0);
        lv_obj_set_style_pad_top(chart, 0, 0);
        lv_obj_set_style_pad_bottom(chart, 0, 0);
        lv_obj_set_style_pad_left(chart, 0, 0);
        lv_obj_set_style_pad_right(chart, 0, 0);
        lv_obj_set_style_line_width(chart, 2, lv_part_t_LV_PART_INDICATOR);
        lv_obj_remove_flag(chart, lv_obj_flag_t_LV_OBJ_FLAG_SCROLLABLE);

        // Single green trace for the ECG waveform
        let ecg_series = lv_chart_add_series(
            chart,
            lv_color_hex(0x4ade80), // bright medical green
            lv_chart_axis_t_LV_CHART_AXIS_PRIMARY_Y,
        );

        // Pre-populate with a realistic P-QRS-T complex that repeats.
        // Each cycle: P wave → QRS complex → T wave → baseline.
        let ecg_wave: [i16; 50] = [
            -2, -1, 0, 2, 3, 3, 2, 0, -1, -2, // P wave
            -2, -1, 0, 2, 5, 10, 20, 40, 60, 80, // QRS upslope
            95, 90, 60, 25, -5, -25, -15, -3, 2, 4, // QRS downslope
            6, 8, 10, 12, 15, 18, 22, 26, 30, 32, // T wave upslope
            30, 25, 18, 10, 5, 2, 0, -1, -2, -3, // T wave downslope
        ];

        for i in 0..100 {
            let val = ecg_wave[i % 50] as i32;
            lv_chart_set_next_value(chart, ecg_series, val);
        }

        // ── HRV label (bottom) ───────────────────────────────
        let hrv_label = lv_label_create(p);
        lv_label_set_text(hrv_label, c"HRV: --- ms".as_ptr());
        lv_obj_set_style_text_color(hrv_label, overlay, 0);
        lv_obj_set_style_text_align(hrv_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(hrv_label, LABEL_W);
        lv_obj_set_pos(hrv_label, CX - LABEL_HALF, scale(390));

        Handles {
            title,
            hr_label,
            r2r_label,
            lead_label,
            chart,
            chart_mask,
            ecg_series,
            hrv_label,
        }
    }
}

pub fn apply_theme(h: &Handles, pal: &ThemePalette) {
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        lv_obj_set_style_text_color(h.title, text, 0);
        lv_obj_set_style_text_color(h.hr_label, text, 0);
        lv_obj_set_style_text_color(h.r2r_label, overlay, 0);
        lv_obj_set_style_text_color(h.lead_label, overlay, 0);
        lv_obj_set_style_border_color(h.chart_mask, overlay, 0);
        lv_obj_set_style_text_color(h.hrv_label, overlay, 0);
    }
}
