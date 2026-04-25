/// Watchface clock calculation helpers.
/// Extracted from the UI render loop so angle math lives alongside the UI.

/// Result of a clock tick: formatted time string and hand angles.
pub struct ClockState {
    pub time: slint::SharedString,
    pub hour_angle: f32,
    pub minute_angle: f32,
    pub elapsed_seconds: i32,
    hours: u32,
    minutes: u32,
}

/// Compute clock state from elapsed seconds since boot.
/// Returns `None` if the time hasn't changed from the previous tick.
pub fn tick(elapsed_seconds: u32, prev: Option<&ClockState>) -> Option<ClockState> {
    let minutes = (elapsed_seconds / 60) % 60;
    let hours = (elapsed_seconds / 3600) % 24;

    if prev.is_some_and(|p| p.hours == hours && p.minutes == minutes) {
        return None;
    }

    let hour_12 = match hours % 12 {
        0 => 12,
        value => value,
    };

    Some(ClockState {
        time: slint::format!("{:02}:{:02}", hours, minutes),
        hour_angle: hour_12 as f32 * 30. + minutes as f32 * 0.5,
        minute_angle: minutes as f32 * 6.,
        elapsed_seconds: elapsed_seconds as i32,
        hours,
        minutes,
    })
}
