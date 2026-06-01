use core::alloc::GlobalAlloc;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use oxivgl::display::LvglBuffers;

use crate::services::rendering::display::{OxivglDisplay, SmartWatchDisplay};
use crate::ui::config::PRODUCTION_UI_SIZE;

/// Screen dimensions in LVGL pixel coordinates.
pub const SCREEN_W: i32 = PRODUCTION_UI_SIZE as i32;
pub const SCREEN_H: i32 = PRODUCTION_UI_SIZE as i32;

/// Buffer size: full screen × 2 bytes/pixel (RGB565).
pub const LVGL_BUF_BYTES: usize = SCREEN_W as usize * SCREEN_H as usize * 2;

/// Channel for brightness updates (0-255). Sent from the LVGL slider callback,
/// consumed by the flush task on the interrupt executor.
pub static BRIGHTNESS_CHANNEL: Channel<CriticalSectionRawMutex, u8, 1> = Channel::new();

/// Wraps [`SmartWatchDisplay`] to inject brightness updates between LVGL draw
/// stripes. The existing oxivgl `flush_frame_buffer` loop calls `show_raw_data`
/// for each LVGL stripe; we check for pending brightness values before each write.
struct BrightnessDisplay {
    display: SmartWatchDisplay,
}

// SAFETY: `SmartWatchDisplay` contains `Spi<Async>` (peripheral — globally
// addressable hardware). The display is only accessed from core 1's interrupt
// executor, never concurrently.
unsafe impl Send for BrightnessDisplay {}

impl oxivgl::flush_pipeline::DisplayOutput for BrightnessDisplay {
    async fn show_raw_data(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        data: &[u8],
    ) -> Result<(), oxivgl::flush_pipeline::UiError> {
        let area = display_driver::Area::new(x, y, w, h);
        let fc = display_driver::bus::FrameControl::new_standalone();
        self.display
            .write_pixels(area, fc, data)
            .await
            .map_err(|_| oxivgl::flush_pipeline::UiError::Display)?;

        // Drain brightness AFTER pixel write — avoids the brightness QSPI
        // command interfering with the current frame's write window.
        // Drain one value per frame (capacity=1); at 30fps this is enough
        // to track slider drags without unnecessary SPI traffic.
        if let Ok(brightness) = BRIGHTNESS_CHANNEL.try_receive() {
            if let Err(_e) = self.display.set_brightness(brightness).await {
                defmt::error!("set_brightness failed");
            }
        }

        Ok(())
    }
}

/// Flush task: owns the display via [`OxivglDisplay`] (which is `Send`),
/// wraps it in a [`BrightnessDisplay`] that handles brightness updates,
/// then delegates to the oxivgl flush pipeline.
#[embassy_executor::task]
pub async fn flush_task(oxivgl_display: OxivglDisplay) -> ! {
    let wrapped = BrightnessDisplay { display: oxivgl_display.0 };
    oxivgl::flush_pipeline::flush_frame_buffer(wrapped).await
}

/// Allocate full-screen LVGL double-buffers on the PSRAM heap.
///
/// Must be called once, then the reference passed to the UI task.
pub fn take_lvgl_buffers() -> &'static mut LvglBuffers<LVGL_BUF_BYTES> {
    use core::alloc::Layout;
    let buf_size = (LVGL_BUF_BYTES + 15) & !15;
    let total = buf_size * 2;
    let layout = unsafe { Layout::from_size_align_unchecked(total, 16) };
    let ptr = unsafe { esp_alloc::HEAP.alloc(layout) };
    if ptr.is_null() {
        panic!("OOM: cannot allocate LVGL full-screen buffers");
    }
    unsafe { core::ptr::write_bytes(ptr, 0, total) };
    // SAFETY: called once, single-threaded on core 1.
    unsafe { &mut *(ptr as *mut LvglBuffers<LVGL_BUF_BYTES>) }
}
