use core::mem;

use oxivgl::enums::ScrollDir;
use oxivgl::style::props;
use oxivgl::widgets::{Obj, Tileview, WidgetError};
use oxivgl_sys::{
    _lv_style_id_t_LV_STYLE_TEXT_COLOR, lv_anim_path_ease_out, lv_color_hex,
    lv_obj_set_style_bg_color, lv_obj_set_style_text_color, lv_obj_set_style_transition,
    lv_style_prop_t, lv_style_transition_dsc_init, lv_style_transition_dsc_t,
};

use super::{ecg, gps, settings, theme, vitals, watchface};

static THEME_PROPS: [lv_style_prop_t; 3] = [
    props::BG_COLOR,
    _lv_style_id_t_LV_STYLE_TEXT_COLOR as lv_style_prop_t,
    0 as lv_style_prop_t,
];

pub struct AppHandles {
    pub panes: [*mut oxivgl_sys::lv_obj_t; 5],
    pub theme_switch: *mut oxivgl_sys::lv_obj_t,
    pub bright_slider: *mut oxivgl_sys::lv_obj_t,
    pub time_label: *mut oxivgl_sys::lv_obj_t,
    pub date_label: *mut oxivgl_sys::lv_obj_t,
    pub battery_label: *mut oxivgl_sys::lv_obj_t,
    pub hr_label: *mut oxivgl_sys::lv_obj_t,
    pub spo2_label: *mut oxivgl_sys::lv_obj_t,
}

pub fn create_tileview(
    screen: &Obj<'static>,
) -> Result<(AppHandles, Tileview<'static>), WidgetError> {
    let tv = Tileview::new(screen)?;
    let mut panes = [core::ptr::null_mut(); 5];

    // Settings pane: no scroll direction — the tileview would otherwise
    // intercept drag events for page navigation, preventing child widgets
    // (like the brightness slider) from receiving indev events. Navigation
    // from settings is handled by a "Back" button instead.
    let p0 = tv.add_tile(0, 0, ScrollDir::NONE);
    let sh = settings::create(&*p0, tv.handle())?;
    panes[0] = p0.handle();

    let p1 = tv.add_tile(1, 0, ScrollDir::LEFT | ScrollDir::RIGHT);
    let wh = watchface::create(&*p1)?;
    panes[1] = p1.handle();

    let p2 = tv.add_tile(2, 0, ScrollDir::LEFT | ScrollDir::RIGHT);
    let vh = vitals::create(&*p2)?;
    panes[2] = p2.handle();

    let p3 = tv.add_tile(3, 0, ScrollDir::LEFT | ScrollDir::RIGHT);
    ecg::create(&*p3)?;
    panes[3] = p3.handle();

    let p4 = tv.add_tile(4, 0, ScrollDir::LEFT);
    gps::create(&*p4)?;
    panes[4] = p4.handle();

    // Initialise a transition descriptor and set it on every pane.
    let mut tr: lv_style_transition_dsc_t = unsafe { mem::zeroed() };
    unsafe {
        lv_style_transition_dsc_init(
            &mut tr,
            THEME_PROPS.as_ptr(),
            Some(lv_anim_path_ease_out),
            300,
            0,
            core::ptr::null_mut(),
        );
    }
    for &p in &panes {
        unsafe {
            lv_obj_set_style_transition(p, &tr, 0);
        }
    }

    tv.set_tile_by_index(1, 0, false);

    Ok((
        AppHandles {
            panes,
            theme_switch: sh.theme_switch,
            bright_slider: sh.bright_slider,
            time_label: wh.time_label,
            date_label: wh.date_label,
            battery_label: wh.battery_label,
            hr_label: vh.hr_label,
            spo2_label: vh.spo2_label,
        },
        tv,
    ))
}

pub fn apply_theme(handles: &AppHandles) {
    let pal = theme::current_palette();
    for &p in &handles.panes {
        unsafe {
            lv_obj_set_style_bg_color(p, lv_color_hex(pal.bg_color), 0);
            lv_obj_set_style_text_color(p, lv_color_hex(pal.text_color), 0);
        }
    }
    if !handles.theme_switch.is_null() {
        unsafe {
            lv_obj_set_style_bg_color(handles.theme_switch, lv_color_hex(pal.accent_color), 2);
        }
    }
    if !handles.bright_slider.is_null() {
        unsafe {
            lv_obj_set_style_bg_color(handles.bright_slider, lv_color_hex(pal.accent_color), 2);
        }
    }
}
