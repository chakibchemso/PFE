/// Production display size (466x466 round AMOLED)
pub const PRODUCTION_UI_SIZE: u16 = 466;

/// Maximum supported viewport size for mask LUT
const MAX_MASK_SIZE: usize = 466;

/// Precomputed round mask: each row stores (start_x, end_x) for valid pixels
pub struct RoundMaskLUT {
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
                row_ranges[y] = (0, 0);
            } else {
                let half_width = libm::sqrtf(max_dx2 as f32) as i32;
                let start_x = (radius - half_width).max(0) as u16;
                let end_x = (radius + half_width).min(size as i32 - 1) as u16;
                row_ranges[y] = (start_x, end_x);
            }
        }

        Self { row_ranges, size }
    }

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
    pub const fn production() -> Self {
        Self {
            panel_width: PRODUCTION_UI_SIZE,
            panel_height: PRODUCTION_UI_SIZE,
            viewport_size: PRODUCTION_UI_SIZE,
            viewport_offset_x: 0,
            viewport_offset_y: 0,
            round_mask: true,
            display_mirror_x: false,
            display_mirror_y: false,
            touch_mirror_x: true,
            touch_mirror_y: true,
            touch_swap_xy: false,
        }
    }

    /// Map touch coordinates to viewport space.
    /// Returns (x, y) in LVGL pixel coordinates, or None if outside the viewport.
    pub fn map_touch_to_viewport(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        if x < self.viewport_offset_x
            || y < self.viewport_offset_y
            || x >= self.viewport_offset_x + self.viewport_size
            || y >= self.viewport_offset_y + self.viewport_size
        {
            return None;
        }

        let local_x = x - self.viewport_offset_x;
        let local_y = y - self.viewport_offset_y;

        if self.round_mask {
            let radius = self.viewport_size as i32 / 2;
            let center = radius;
            let dx = local_x as i32 - center;
            let dy = local_y as i32 - center;
            if dx * dx + dy * dy > radius * radius {
                return None;
            }
        }

        Some((local_x, local_y))
    }
}
