pub mod display;
pub mod lvgl_alloc;
pub mod task;

pub use display::{OxivglDisplay, SmartWatchDisplay, init_display};
pub use task::{LVGL_BUF_BYTES, SCREEN_H, SCREEN_W, flush_task, take_lvgl_buffers};

use embassy_executor::SendSpawner;

/// Spawn the flush task on the interrupt executor.
///
/// The flush task runs at interrupt priority and forwards LVGL pixel data to the
/// CO5300 display via DMA. Must be spawned before the LVGL render task.
pub fn register_flush(spawner: &SendSpawner, display: OxivglDisplay) {
    spawner.spawn(flush_task(display).unwrap());
}
