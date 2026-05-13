/// GPS page logic: formats fix data for display and computes compass needle angle.
use crate::app::bus::GpsFix;
use crate::drivers::gps::format_coords;

/// Result for the GPS display state.
pub struct GpsState {
    pub heading: f32,
    pub has_fix: bool,
    pub coords: slint::SharedString,
    pub speed: slint::SharedString,
    pub sats: slint::SharedString,
    pub altitude: slint::SharedString,
}

impl GpsState {
    /// Create a searching state (no GPS fix yet).
    pub fn searching() -> Self {
        Self {
            heading: 0.0,
            has_fix: false,
            coords: slint::SharedString::from("--"),
            speed: slint::SharedString::from("-- km/h"),
            sats: slint::SharedString::from("-- sats"),
            altitude: slint::SharedString::from("-- m"),
        }
    }

    /// Create state from a GPS fix.
    pub fn from_fix(fix: &GpsFix) -> Self {
        if !fix.has_fix {
            return Self::searching();
        }

        let (coords_buf, coords_len) = format_coords(fix.lat, fix.lon);
        let coords_str = core::str::from_utf8(&coords_buf[..coords_len]).unwrap_or("--");

        Self {
            heading: fix.heading_deg,
            has_fix: true,
            coords: slint::SharedString::from(coords_str),
            speed: slint::format!("{:.1} km/h", fix.speed_kmh),
            sats: slint::format!("{} sats", fix.satellites),
            altitude: slint::format!("{:.1} m", fix.altitude_m),
        }
    }
}
