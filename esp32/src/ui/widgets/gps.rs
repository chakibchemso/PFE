/// GPS page logic: compass needle angle and status formatting.

/// Result for the GPS display state.
pub struct GpsState {
    pub needle_angle: f32,
    pub has_fix: bool,
    pub status_text: slint::SharedString,
    pub status_color: slint::Color,
}

impl GpsState {
    /// Create a searching state (no GPS fix yet).
    pub fn searching() -> Self {
        Self {
            needle_angle: 28.,
            has_fix: false,
            status_text: slint::format!("Searching"),
            status_color: slint::Color::from_rgb_u8(250, 179, 135), // peach
        }
    }

    /// Create a locked state with given heading.
    pub fn locked(heading_deg: f32) -> Self {
        Self {
            needle_angle: heading_deg,
            has_fix: true,
            status_text: slint::format!("Fix locked"),
            status_color: slint::Color::from_rgb_u8(166, 227, 161), // green
        }
    }
}
