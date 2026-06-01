use oxivgl::widgets::*;

use crate::services::rendering::task::SCREEN_W;

pub struct Handles {
    pub time_label: *mut oxivgl_sys::lv_obj_t,
    pub date_label: *mut oxivgl_sys::lv_obj_t,
    pub battery_label: *mut oxivgl_sys::lv_obj_t,
}

pub fn create(parent: &impl AsLvHandle) -> Result<Handles, WidgetError> {
    let time = Label::new(parent)?;
    time.text("00:00")
        .align(Align::Center, 0, -60)
        .width(SCREEN_W - 20);
    let time_h = time.handle();
    let _ = Child::new(time);

    let date = Label::new(parent)?;
    date.text("--- -- ----")
        .align(Align::Center, 0, -10);
    let date_h = date.handle();
    let _ = Child::new(date);

    let bat = Label::new(parent)?;
    bat.text("100%")
        .align(Align::Center, 0, 50);
    let bat_h = bat.handle();
    let _ = Child::new(bat);

    Ok(Handles { time_label: time_h, date_label: date_h, battery_label: bat_h })
}
