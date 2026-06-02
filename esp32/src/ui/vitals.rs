use core::ffi::CStr;
use core::ptr::NonNull;

use lv_bevy_ecs::support::Align;
use lv_bevy_ecs::sys::lv_obj_t;
use lv_bevy_ecs::widgets::{Label, Wdg};

use crate::services::rendering::task::SCREEN_W;

pub struct Handles {
    pub hr_label: NonNull<lv_obj_t>,
    pub spo2_label: NonNull<lv_obj_t>,
}

pub fn create(parent: &mut Wdg) -> Handles {
    let mut hr = Label::new();
    hr.set_text(lv_bevy_ecs::cstr!("72 BPM"));
    hr.set_width((SCREEN_W - 20).into());
    align_to(&mut hr, parent, Align::Center, 0, -40);
    hr.set_parent(parent);
    let hr_h = NonNull::new(hr.raw_mut()).expect("hr handle");
    let _ = hr.leak();

    let mut spo2 = Label::new();
    spo2.set_text(lv_bevy_ecs::cstr!("98 % SpO2"));
    align_to(&mut spo2, parent, Align::Center, 0, 10);
    spo2.set_parent(parent);
    let spo2_h = NonNull::new(spo2.raw_mut()).expect("spo2 handle");
    let _ = spo2.leak();

    Handles {
        hr_label: hr_h,
        spo2_label: spo2_h,
    }
}

fn align_to(w: &mut Wdg, _parent: &Wdg, align: Align, x_ofs: i32, y_ofs: i32) {
    let a: lv_bevy_ecs::sys::lv_align_t = align.into();
    unsafe {
        lv_bevy_ecs::sys::lv_obj_align(w.raw_mut(), a, x_ofs, y_ofs);
    }
}
