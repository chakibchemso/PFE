pub mod display;
pub mod lvgl_alloc;
pub mod task;

pub use display::{SendDisplay, SmartWatchDisplay, init_display};
pub use task::{
    BRIGHTNESS_CHANNEL, DISPLAY_READY, SCREEN_H, SCREEN_W, flush_task, init_lvgl_display,
};
