use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::watch::Receiver;
use embassy_time::{Duration, Instant, Timer};
use slint::ComponentHandle;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PhysicalRegion, Rgb565Pixel};

use super::config::{RenderConfig, RoundMaskLUT};
use super::widgets::watchface;
use super::{SmartWatchDisplay, SmartWatchUI};

/// Shared window handle for the touch task to dispatch events
pub type SharedWindow = Mutex<CriticalSectionRawMutex, RefCell<Option<Rc<MinimalSoftwareWindow>>>>;

/// Single framebuffer with dirty-rect encoding and per-rect transfer.
pub struct Framebuffer {
    pixels: Vec<Rgb565Pixel>,
    tx_buf: Vec<u8>,
    size: usize,
}

impl Framebuffer {
    pub fn new(size: usize) -> Self {
        let pixel_count = size * size;
        Self {
            pixels: vec![Rgb565Pixel(0); pixel_count],
            tx_buf: vec![0u8; pixel_count * 2],
            size,
        }
    }

    pub fn buffer_mut(&mut self) -> &mut [Rgb565Pixel] {
        &mut self.pixels[..]
    }

    /// Render dirty rects, apply mask, encode to bytes.
    /// Returns (render_us, changed_rects).
    pub fn render(
        &mut self,
        window: &MinimalSoftwareWindow,
        viewport_size: usize,
        config: RenderConfig,
        mask_lut: &RoundMaskLUT,
    ) -> (u64, Vec<(u32, u32, u32, u32)>) {
        let mut changed_rects: Vec<(u32, u32, u32, u32)> = Vec::new();

        let render_start = Instant::now();
        window.draw_if_needed(|renderer| {
            let dirty = renderer.render(self.buffer_mut(), viewport_size);
            changed_rects = Self::flush_dirty(
                &dirty,
                config,
                mask_lut,
                &mut self.pixels,
                &mut self.tx_buf,
                self.size,
            );
        });
        ((Instant::now() - render_start).as_micros(), changed_rects)
    }

    /// Transfer each dirty rect individually to avoid sending unchanged pixels.
    /// Returns total transfer time in microseconds.
    pub async fn transfer_batch(
        &self,
        config: RenderConfig,
        display: &mut SmartWatchDisplay,
        changed_rects: &[(u32, u32, u32, u32)],
    ) -> u64 {
        if changed_rects.is_empty() {
            return 0;
        }

        let transfer_start = Instant::now();

        for &(x, y, w, h) in changed_rects {
            if w == 0 || h == 0 {
                continue;
            }

            let mut pixel_bytes = Vec::with_capacity((w * h * 2) as usize);
            for row in 0..h {
                let row_offset = ((y + row) as usize) * self.size + x as usize;
                let byte_offset = row_offset * 2;
                let row_len = (w * 2) as usize;
                pixel_bytes.extend_from_slice(&self.tx_buf[byte_offset..byte_offset + row_len]);
            }

            let panel_x = config.viewport_offset_x as u32 + x;
            let panel_y = config.viewport_offset_y as u32 + y;
            let _ = display
                .show_raw_data(
                    panel_x as u16,
                    panel_y as u16,
                    w as u16,
                    h as u16,
                    &pixel_bytes,
                )
                .await;
        }

        (Instant::now() - transfer_start).as_micros()
    }

    fn flush_dirty(
        dirty_region: &PhysicalRegion,
        config: RenderConfig,
        mask_lut: &RoundMaskLUT,
        pixels: &mut [Rgb565Pixel],
        tx_buf: &mut [u8],
        size: usize,
    ) -> Vec<(u32, u32, u32, u32)> {
        let mut changed_rects: Vec<(u32, u32, u32, u32)> = Vec::new();

        for (pos, dsize) in dirty_region.iter() {
            let x = pos.x as u32;
            let y = pos.y as u32;
            let w = dsize.width;
            let h = dsize.height;

            if w == 0 || h == 0 {
                continue;
            }

            config.apply_round_mask_rect(mask_lut, pixels, x, y, w, h);

            for row in 0..h {
                let row_offset = ((y + row) as usize) * size + x as usize;
                let byte_offset = row_offset * 2;
                let pixel_row = &pixels[row_offset..row_offset + w as usize];
                let byte_row = &mut tx_buf[byte_offset..byte_offset + w as usize * 2];
                for (col, pixel) in pixel_row.iter().enumerate() {
                    let be = pixel.0.to_be_bytes();
                    let bo = col * 2;
                    byte_row[bo] = be[0];
                    byte_row[bo + 1] = be[1];
                }
            }

            changed_rects.push((x, y, w, h));
        }

        changed_rects
    }
}

/// Main UI task: handles vitals, clock, rendering, and display transfer.
/// Touch is handled independently by touch_task.
#[embassy_executor::task]
pub async fn ui_task(
    config: RenderConfig,
    mut display: SmartWatchDisplay,
    shared_window: &'static SharedWindow,
    window: Rc<MinimalSoftwareWindow>,
    mut vitals_receiver: Receiver<'static, CriticalSectionRawMutex, (f32, f32, f32), 2>,
) {
    let viewport_size = config.viewport_size as usize;
    let mut fb = Framebuffer::new(viewport_size);

    let mask_lut = RoundMaskLUT::new(config.viewport_size);

    let ui = SmartWatchUI::new().unwrap();
    ui.set_time(slint::format!("12:00"));
    ui.set_bpm(slint::format!("--"));
    ui.set_spo2(slint::format!("--"));
    ui.set_hour_angle(0.);
    ui.set_minute_angle(0.);
    ui.set_wifi_connected(true);
    ui.set_ascon_secure(true);
    ui.set_gps_fix(false);
    ui.set_dark_mode(true);
    ui.set_show_fps(false);
    ui.show().unwrap();

    window.request_redraw();

    // Register window handle so touch_task can dispatch events
    {
        let guard = shared_window.lock().await;
        guard.replace(Some(window.clone()));
    }

    let mut clock_state: Option<watchface::ClockState> = None;
    let start_ms = Instant::now().as_millis();

    loop {
        // Update clock via extracted widget logic
        let elapsed_seconds = ((Instant::now().as_millis() - start_ms) / 1000) as u32;
        if let Some(state) = watchface::tick(elapsed_seconds, clock_state.as_ref()) {
            ui.set_time(state.time.clone());
            ui.set_hour_angle(state.hour_angle);
            ui.set_minute_angle(state.minute_angle);
            ui.set_elapsed_seconds(state.elapsed_seconds);
            clock_state = Some(state);
            window.request_redraw();
        }

        // Update vitals
        if let Some((bpm, spo2, _temp)) = vitals_receiver.try_changed() {
            ui.set_bpm(slint::format!("{:.0}", bpm));
            ui.set_spo2(slint::format!("{:.0}", spo2));
            window.request_redraw();
        }

        slint::platform::update_timers_and_animations();

        let (_render_us, changed_rects) = fb.render(&window, viewport_size, config, &mask_lut);

        let _transfer_us = fb
            .transfer_batch(config, &mut display, &changed_rects)
            .await;

        Timer::after(Duration::from_millis(0)).await;
    }
}
