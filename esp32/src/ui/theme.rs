use core::ptr::addr_of;
use core::sync::atomic::{AtomicU8, Ordering};

use lv_bevy_ecs::functions::lv_color_hex;
use lv_bevy_ecs::sys::{
    LV_RADIUS_CIRCLE, lv_button_class, lv_label_class, lv_obj_check_type, lv_obj_get_child,
    lv_obj_get_child_count, lv_obj_set_style_bg_color, lv_obj_set_style_bg_opa,
    lv_obj_set_style_border_color, lv_obj_set_style_radius, lv_obj_set_style_text_color, lv_obj_t,
    lv_part_t_LV_PART_INDICATOR, lv_part_t_LV_PART_KNOB, lv_slider_class,
    lv_state_t_LV_STATE_CHECKED, lv_switch_class, lv_tileview_tile_class,
};

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub bg_color: u32,
    pub text_color: u32,
    pub surface_color: u32,
    pub overlay_color: u32,
    pub accent_color: u32,
    pub healthy_color: u32,
    pub unhealthy_color: u32,
}

pub const LATTE: ThemePalette = ThemePalette {
    bg_color: 0xeff1f5,
    text_color: 0x4c4f69,
    surface_color: 0xe6e9ef,
    overlay_color: 0x9ca0b0,
    accent_color: 0x1e66f5,
    healthy_color: 0x40a02b,
    unhealthy_color: 0xd20f39,
};

pub const MOCHA: ThemePalette = ThemePalette {
    bg_color: 0x1e1e2e,
    text_color: 0xcdd6f4,
    surface_color: 0x313244,
    overlay_color: 0x6c7086,
    accent_color: 0x89b4fa,
    healthy_color: 0xa6e3a1,
    unhealthy_color: 0xf38ba8,
};

pub static CURRENT_THEME: AtomicU8 = AtomicU8::new(1);
pub static CURRENT_BRIGHTNESS: AtomicU8 = AtomicU8::new(80);

pub fn current_palette() -> &'static ThemePalette {
    match CURRENT_THEME.load(Ordering::Relaxed) {
        0 => &LATTE,
        _ => &MOCHA,
    }
}

/// Apply the full palette to a pane (tileview tile) and all its descendants.
pub fn apply_to_pane(pane: *mut lv_obj_t, pal: &ThemePalette) {
    apply_to_widget(pane, pal);
    apply_to_children(pane, pal);
}

/// Walk every child of `parent` and apply type-specific colours, then recurse.
fn apply_to_children(parent: *mut lv_obj_t, pal: &ThemePalette) {
    let count = unsafe { lv_obj_get_child_count(parent) };
    for i in 0..count {
        let child = unsafe { lv_obj_get_child(parent, i as i32) };
        if child.is_null() {
            continue;
        }
        apply_to_widget(child, pal);
        apply_to_children(child, pal);
    }
}

/// Set colours on a single widget according to its LVGL class.
fn apply_to_widget(obj: *mut lv_obj_t, pal: &ThemePalette) {
    let bg = lv_color_hex(pal.bg_color);
    let text = lv_color_hex(pal.text_color);
    let surface = lv_color_hex(pal.surface_color);
    let accent = lv_color_hex(pal.accent_color);
    let overlay = lv_color_hex(pal.overlay_color);

    unsafe {
        if lv_obj_check_type(obj, addr_of!(lv_tileview_tile_class)) {
            lv_obj_set_style_bg_color(obj, bg, 0);
            lv_obj_set_style_bg_opa(obj, 255, 0);
            lv_obj_set_style_text_color(obj, text, 0);
        } else if lv_obj_check_type(obj, addr_of!(lv_button_class)) {
            lv_obj_set_style_bg_color(obj, accent, 0);
            lv_obj_set_style_text_color(obj, text, 0);
            lv_obj_set_style_border_color(obj, overlay, 0);
            lv_obj_set_style_radius(obj, LV_RADIUS_CIRCLE as i32, 0);
        } else if lv_obj_check_type(obj, addr_of!(lv_slider_class)) {
            lv_obj_set_style_bg_color(obj, surface, 0);
            lv_obj_set_style_bg_color(obj, accent, lv_part_t_LV_PART_INDICATOR);
            lv_obj_set_style_bg_color(obj, accent, lv_part_t_LV_PART_KNOB);
        } else if lv_obj_check_type(obj, addr_of!(lv_switch_class)) {
            lv_obj_set_style_bg_color(obj, surface, 0);
            lv_obj_set_style_bg_color(
                obj,
                accent,
                lv_part_t_LV_PART_INDICATOR | lv_state_t_LV_STATE_CHECKED,
            );
            lv_obj_set_style_bg_opa(
                obj,
                255,
                lv_part_t_LV_PART_INDICATOR | lv_state_t_LV_STATE_CHECKED,
            );
            lv_obj_set_style_bg_color(obj, text, lv_part_t_LV_PART_KNOB);
        } else if lv_obj_check_type(obj, addr_of!(lv_label_class)) {
            // Labels inherit text_color from their parent automatically
        }
    }
}
