use core::ffi::CStr;
use core::ptr::NonNull;

use lv_bevy_ecs::support::Align;
use lv_bevy_ecs::sys::lv_obj_t;
use lv_bevy_ecs::widgets::{Label, Wdg};

use crate::services::rendering::task::SCREEN_W;

pub struct Handles {
    pub time_label: NonNull<lv_obj_t>,
    pub date_label: NonNull<lv_obj_t>,
    pub battery_label: NonNull<lv_obj_t>,
}

pub fn create(parent: &mut Wdg) -> Handles {
    let mut time = Label::new();
    time.set_text(lv_bevy_ecs::cstr!("00:00"));
    time.set_width((SCREEN_W - 20).into());
    align_to(&mut time, parent, Align::Center, 0, -60);
    time.set_parent(parent);
    let time_h = NonNull::new(time.raw_mut()).expect("time handle");
    let _ = time.leak();

    let mut date = Label::new();
    date.set_text(lv_bevy_ecs::cstr!("--- -- ----"));
    align_to(&mut date, parent, Align::Center, 0, -10);
    date.set_parent(parent);
    let date_h = NonNull::new(date.raw_mut()).expect("date handle");
    let _ = date.leak();

    let mut bat = Label::new();
    bat.set_text(lv_bevy_ecs::cstr!("100%"));
    align_to(&mut bat, parent, Align::Center, 0, 50);
    bat.set_parent(parent);
    let bat_h = NonNull::new(bat.raw_mut()).expect("battery handle");
    let _ = bat.leak();

    Handles {
        time_label: time_h,
        date_label: date_h,
        battery_label: bat_h,
    }
}

fn align_to(w: &mut Wdg, _parent: &Wdg, align: Align, x_ofs: i32, y_ofs: i32) {
    let a: lv_bevy_ecs::sys::lv_align_t = align.into();
    unsafe {
        lv_bevy_ecs::sys::lv_obj_align(w.raw_mut(), a, x_ofs, y_ofs);
    }
}
