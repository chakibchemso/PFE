//! Vitals formatting helpers for Slint UI.
//!
//! Constants and formatters for mapping BPM/SpO2/temp to Slint properties.

/// Format a BPM value for display.
pub fn format_bpm(bpm: u8) -> slint::SharedString {
    slint::format!("{}", bpm)
}

/// Format an SpO2 value for display.
pub fn format_spo2(spo2: u8) -> slint::SharedString {
    slint::format!("{}", spo2)
}
