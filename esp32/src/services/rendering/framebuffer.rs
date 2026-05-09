use alloc::vec;
use alloc::vec::Vec;
use defmt::warn;
use display_driver::Area;
use display_driver::bus::FrameControl;
use embassy_time::Instant;
use slint::platform::software_renderer::{MinimalSoftwareWindow, Rgb565Pixel};

use super::display::SmartWatchDisplay;
use crate::ui::config::RenderConfig;

/// Single framebuffer with dirty-rect encoding and per-rect transfer.
pub struct Framebuffer {
    pixels: Vec<Rgb565Pixel>,
    size: usize,
}

impl Framebuffer {
    pub fn new(size: usize) -> Self {
        let pixel_count = size * size;
        Self {
            pixels: vec![Rgb565Pixel(0); pixel_count],
            size,
        }
    }

    pub fn buffer_mut(&mut self) -> &mut [Rgb565Pixel] {
        &mut self.pixels[..]
    }

    /// Render the UI if needed. Returns true if something was drawn.
    pub fn render(&mut self, window: &MinimalSoftwareWindow, viewport_size: usize) -> bool {
        let mut drew_anything = false;
        window.draw_if_needed(|renderer| {
            renderer.render(self.buffer_mut(), viewport_size);
            drew_anything = true;
        });
        drew_anything
    }

    /// Transfer the entire framebuffer to the display.
    pub async fn transfer_full(
        &mut self,
        config: RenderConfig,
        display: &mut SmartWatchDisplay,
    ) -> u64 {
        let transfer_start = Instant::now();

        // Byte-swap in place for SPI transmission (Little Endian -> Big Endian)
        for pixel in &mut self.pixels {
            pixel.0 = pixel.0.swap_bytes();
        }

        // Safe cast to byte slice
        let pixel_bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, self.pixels.len() * 2)
        };

        let area = Area::new(
            config.viewport_offset_x,
            config.viewport_offset_y,
            self.size as u16,
            self.size as u16,
        );

        let frame_control = FrameControl::new_standalone();

        if display
            .write_pixels(area, frame_control, pixel_bytes)
            .await
            .is_err()
        {
            warn!("write_pixels failed");
        }

        // Swap back to Little Endian for Slint to use in the next frame
        for pixel in &mut self.pixels {
            pixel.0 = pixel.0.swap_bytes();
        }

        (Instant::now() - transfer_start).as_micros()
    }
}
