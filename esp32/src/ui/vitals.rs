use oxivgl::widgets::*;

use crate::services::rendering::task::SCREEN_W;

pub struct Handles {
    pub hr_label: *mut oxivgl_sys::lv_obj_t,
    pub spo2_label: *mut oxivgl_sys::lv_obj_t,
}

pub fn create(parent: &impl AsLvHandle) -> Result<Handles, WidgetError> {
    let hr = Label::new(parent)?;
    hr.text("72 BPM")
        .align(Align::Center, 0, -40)
        .width(SCREEN_W - 20);
    let hr_h = hr.handle();
    let _ = Child::new(hr);

    let spo2 = Label::new(parent)?;
    spo2.text("98 % SpO2")
        .align(Align::Center, 0, 10);
    let spo2_h = spo2.handle();
    let _ = Child::new(spo2);

    Ok(Handles { hr_label: hr_h, spo2_label: spo2_h })
}
