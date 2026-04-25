use slint::LogicalPosition;

use crate::ui::PRODUCTION_UI_SIZE;

/// Maximum supported viewport size for mask LUT
const MAX_MASK_SIZE: usize = 466;

/// Precomputed round mask: each row stores (start_x, end_x) for valid pixels
/// This avoids per-row sqrt computation during rendering
pub struct RoundMaskLUT {
    /// For each row y, stores (start_x, end_x) inclusive range of valid pixels
    pub row_ranges: [(u16, u16); MAX_MASK_SIZE],
    pub size: u16,
}

impl RoundMaskLUT {
    pub fn new(size: u16) -> Self {
        let mut row_ranges: [(u16, u16); MAX_MASK_SIZE] = [(0, 0); MAX_MASK_SIZE];
        let radius = size as i32 / 2;
        let radius2 = radius * radius;

        for y in 0..size as usize {
            let dy = y as i32 - radius;
            let dy2 = dy * dy;
            let max_dx2 = radius2 - dy2;

            if max_dx2 < 0 {
                row_ranges[y] = (0, 0); // Entire row outside circle
            } else {
                let half_width = libm::sqrtf(max_dx2 as f32) as i32;
                let start_x = (radius - half_width).max(0) as u16;
                let end_x = (radius + half_width).min(size as i32 - 1) as u16;
                row_ranges[y] = (start_x, end_x);
            }
        }

        Self { row_ranges, size }
    }

    /// Check if a pixel is inside the round mask
    #[inline]
    pub fn is_inside(&self, x: u32, y: u32) -> bool {
        if y >= self.size as u32 || x >= self.size as u32 {
            return false;
        }
        let (start, end) = self.row_ranges[y as usize];
        x >= start as u32 && x <= end as u32
    }
}

/// Display and touch configuration
#[derive(Clone, Copy)]
pub struct RenderConfig {
    pub panel_width: u16,
    pub panel_height: u16,
    pub viewport_size: u16,
    pub viewport_offset_x: u16,
    pub viewport_offset_y: u16,
    pub round_mask: bool,
    pub display_mirror_x: bool,
    pub display_mirror_y: bool,
    pub touch_mirror_x: bool,
    pub touch_mirror_y: bool,
    pub touch_swap_xy: bool,
}

impl RenderConfig {
    /// Config for dev boards with ST7796 (320x480, centered 320x320 viewport)
    pub const fn dev_st7796() -> Self {
        Self {
            panel_width: 320,
            panel_height: 480,
            viewport_size: 320,
            viewport_offset_x: 0,
            viewport_offset_y: 80,
            round_mask: true,
            display_mirror_x: false,
            display_mirror_y: true,
            touch_mirror_x: true,
            touch_mirror_y: true,
            touch_swap_xy: false,
        }
    }

    /// Config for production round display (466x466)
    pub const fn production_round() -> Self {
        Self {
            panel_width: PRODUCTION_UI_SIZE,
            panel_height: PRODUCTION_UI_SIZE,
            viewport_size: PRODUCTION_UI_SIZE,
            viewport_offset_x: 0,
            viewport_offset_y: 0,
            round_mask: true,
            display_mirror_x: false,
            display_mirror_y: false,
            touch_mirror_x: false,
            touch_mirror_y: false,
            touch_swap_xy: false,
        }
    }

    /// Map touch coordinates to viewport space, applying transformations and mask
    pub fn map_touch_to_viewport(&self, x: u16, y: u16) -> Option<LogicalPosition> {
        let mut x = x as i32;
        let mut y = y as i32;

        if self.touch_swap_xy {
            core::mem::swap(&mut x, &mut y);
        }
        if self.touch_mirror_x {
            x = self.panel_width as i32 - 1 - x;
        }
        if self.touch_mirror_y {
            y = self.panel_height as i32 - 1 - y;
        }

        let local_x = x - self.viewport_offset_x as i32;
        let local_y = y - self.viewport_offset_y as i32;
        if local_x < 0
            || local_y < 0
            || local_x >= self.viewport_size as i32
            || local_y >= self.viewport_size as i32
        {
            return None;
        }

        if self.round_mask {
            let radius = self.viewport_size as i32 / 2;
            let center = radius;
            let dx = local_x - center;
            let dy = local_y - center;
            if dx * dx + dy * dy > radius * radius {
                return None;
            }
        }

        Some(LogicalPosition::new(local_x as f32, local_y as f32))
    }

    /// Apply round mask to a specific rectangular region of the framebuffer,
    /// zeroing pixels outside the circle using precomputed LUT.
    pub fn apply_round_mask_rect(
        &self,
        mask_lut: &RoundMaskLUT,
        pixels: &mut [slint::platform::software_renderer::Rgb565Pixel],
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) {
        if !self.round_mask {
            return;
        }

        let black = slint::platform::software_renderer::Rgb565Pixel(0);

        for row in 0..h {
            let vy = y + row;
            if vy >= self.viewport_size as u32 {
                continue;
            }

            let (start_x, end_x) = mask_lut.row_ranges[vy as usize];
            let row_offset = (vy as usize) * self.viewport_size as usize;

            // Only black pixels in the rect that are outside the circle
            for col in 0..w {
                let vx = x + col;
                if vx < start_x as u32 || vx > end_x as u32 {
                    pixels[row_offset + vx as usize] = black;
                }
            }
        }
    }
}
