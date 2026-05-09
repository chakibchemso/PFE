pub mod config;
pub mod widgets;

slint::include_modules!();

pub use config::RenderConfig;
pub use widgets::gps::GpsState;
pub use widgets::watchface::{ClockState, tick as clock_tick};
