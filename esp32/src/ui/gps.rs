use core::ffi::CStr;
use lv_bevy_ecs::support::Align;
use lv_bevy_ecs::widgets::{Label, Wdg};

pub fn create(parent: &mut Wdg) {
    let mut lbl = Label::new();
    lbl.set_text(lv_bevy_ecs::cstr!("GPS"));
    align_to(&mut lbl, parent, Align::Center, 0, 0);
    lbl.set_parent(parent);
    let _ = lbl.leak();
}

fn align_to(w: &mut Wdg, _parent: &Wdg, align: Align, x_ofs: i32, y_ofs: i32) {
    let a: lv_bevy_ecs::sys::lv_align_t = align.into();
    unsafe {
        lv_bevy_ecs::sys::lv_obj_align(w.raw_mut(), a, x_ofs, y_ofs);
    }
}
