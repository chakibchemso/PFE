//! GPS data display pane — shows fix status, coordinates, speed, heading,
//! altitude, satellite count, and fix quality on the LVGL tileview.

use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::*;

use super::geom::{CX, scale};
use super::theme::{ThemePalette, current_palette};

const TITLE_Y: i32 = scale(30);
const LABEL_W: i32 = scale(400);
const LABEL_HALF: i32 = LABEL_W / 2;

/// Handles to all GPS pane widgets that are updated at runtime.
pub struct Handles {
    pub title: *mut lv_obj_t,
    pub fix_led: *mut lv_obj_t,
    pub fix_label: *mut lv_obj_t,
    pub coord_label: *mut lv_obj_t,
    pub speed_label: *mut lv_obj_t,
    pub heading_label: *mut lv_obj_t,
    pub altitude_label: *mut lv_obj_t,
    pub sats_label: *mut lv_obj_t,
    pub quality_label: *mut lv_obj_t,
}

/// Create all GPS pane widgets inside `parent` (the tileview tile).
pub fn create(parent: &mut lv_bevy_ecs::widgets::Wdg) -> Handles {
    let p = parent.raw_mut();
    let pal = current_palette();
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        // ── Title ────────────────────────────────────────────
        let title = lv_label_create(p);
        lv_label_set_text(title, c"GPS".as_ptr());
        lv_obj_set_style_text_color(title, text, 0);
        lv_obj_set_style_text_align(title, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(title, LABEL_W);
        lv_obj_set_pos(title, CX - LABEL_HALF, TITLE_Y);

        // ── Fix status LED + label ───────────────────────────
        let fix_led = lv_led_create(p);
        lv_obj_set_pos(fix_led, scale(60), scale(75));
        lv_obj_set_size(fix_led, scale(14), scale(14));
        lv_led_set_color(fix_led, lv_color_hex(0x6c7086));
        lv_led_off(fix_led);

        let fix_label = lv_label_create(p);
        lv_label_set_text(fix_label, c"No Fix".as_ptr());
        lv_obj_set_style_text_color(fix_label, text, 0);
        lv_obj_align_to(
            fix_label,
            fix_led,
            lv_align_t_LV_ALIGN_OUT_RIGHT_MID,
            scale(10),
            0,
        );

        // ── Coordinates ──────────────────────────────────────
        let coord_label = lv_label_create(p);
        lv_label_set_text(coord_label, c"--°--'--- --°--'---".as_ptr());
        lv_obj_set_style_text_color(coord_label, overlay, 0);
        lv_obj_set_style_text_align(coord_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(coord_label, LABEL_W);
        lv_obj_set_pos(coord_label, CX - LABEL_HALF, scale(125));

        // ── Speed + Heading (two columns) ────────────────────
        let speed_label = lv_label_create(p);
        lv_label_set_text(speed_label, c"-- km/h".as_ptr());
        lv_obj_set_style_text_color(speed_label, text, 0);
        lv_obj_set_style_text_align(speed_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(speed_label, scale(180));
        lv_obj_set_pos(speed_label, scale(60), scale(180));

        let heading_label = lv_label_create(p);
        lv_label_set_text(heading_label, c"---°".as_ptr());
        lv_obj_set_style_text_color(heading_label, text, 0);
        lv_obj_set_style_text_align(heading_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(heading_label, scale(180));
        lv_obj_set_pos(heading_label, scale(250), scale(180));

        // ── Altitude + Satellites ────────────────────────────
        let altitude_label = lv_label_create(p);
        lv_label_set_text(altitude_label, c"-- m".as_ptr());
        lv_obj_set_style_text_color(altitude_label, text, 0);
        lv_obj_set_style_text_align(altitude_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(altitude_label, scale(180));
        lv_obj_set_pos(altitude_label, scale(60), scale(230));

        let sats_label = lv_label_create(p);
        lv_label_set_text(sats_label, c"Sats: --".as_ptr());
        lv_obj_set_style_text_color(sats_label, text, 0);
        lv_obj_set_style_text_align(sats_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(sats_label, scale(180));
        lv_obj_set_pos(sats_label, scale(250), scale(230));

        // ── Fix quality ──────────────────────────────────────
        let quality_label = lv_label_create(p);
        lv_label_set_text(quality_label, c"Qual: --".as_ptr());
        lv_obj_set_style_text_color(quality_label, overlay, 0);
        lv_obj_set_style_text_align(quality_label, lv_text_align_t_LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_width(quality_label, LABEL_W);
        lv_obj_set_pos(quality_label, CX - LABEL_HALF, scale(280));

        Handles {
            title,
            fix_led,
            fix_label,
            coord_label,
            speed_label,
            heading_label,
            altitude_label,
            sats_label,
            quality_label,
        }
    }
}

/// Re-apply the current theme palette to all GPS pane widgets.
pub fn apply_theme(h: &Handles, pal: &ThemePalette) {
    let text = lv_color_hex(pal.text_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        lv_obj_set_style_text_color(h.title, text, 0);
        lv_obj_set_style_text_color(h.fix_label, text, 0);
        lv_obj_set_style_text_color(h.coord_label, overlay, 0);
        lv_obj_set_style_text_color(h.speed_label, text, 0);
        lv_obj_set_style_text_color(h.heading_label, text, 0);
        lv_obj_set_style_text_color(h.altitude_label, text, 0);
        lv_obj_set_style_text_color(h.sats_label, text, 0);
        lv_obj_set_style_text_color(h.quality_label, overlay, 0);
    }
}
