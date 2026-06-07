//! LVGL display init, async DMA flush pipeline, and render loop.
//!
//! The flush pipeline mirrors the working oxivgl pattern:
//!   flush_callback (LVGL render thread) → DRAW_OPERATION channel →
//!   flush_task (interrupt executor) → DMA transfer → FLUSH_OPERATION →
//!   wait_callback → lv_display_flush_ready

use core::ffi::c_void;
use core::slice::from_raw_parts;

use defmt::trace;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use esp_hal::gpio::Input;

use crate::services::rendering::display::SendDisplay;
use crate::ui::config::PRODUCTION_UI_SIZE;
use crate::utils::SendWrap;

/// Screen dimensions in LVGL pixel coordinates.
pub const SCREEN_W: i32 = PRODUCTION_UI_SIZE as i32;
pub const SCREEN_H: i32 = PRODUCTION_UI_SIZE as i32;

/// Full-screen buffer byte size (466×466×2 for RGB565).
const BUF_BYTES: usize = PRODUCTION_UI_SIZE as usize * PRODUCTION_UI_SIZE as usize * 2;

/// Channel for brightness updates (0-255). Sent from the LVGL slider callback,
/// consumed by the flush task on the interrupt executor.
pub static BRIGHTNESS_CHANNEL: Channel<CriticalSectionRawMutex, u8, 1> = Channel::new();

// ── Async flush pipeline (same pattern as working oxivgl flush_pipeline.rs) ──

/// Wrapper to make `*mut lv_display_t` Sendable through embassy channels.
/// SAFETY: the pointer is only used on a single core, single executor.
#[derive(Clone, Copy)]
struct DisplayPtr(*mut lv_bevy_ecs::sys::lv_display_t);
unsafe impl Send for DisplayPtr {}

struct DrawOp {
    disp: DisplayPtr,
    data: &'static [u8],
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

// SAFETY: DrawOp is only sent through a channel from LVGL render task to
// flush task; no concurrent access.
unsafe impl Send for DrawOp {}

static DRAW: Channel<CriticalSectionRawMutex, DrawOp, 1> = Channel::new();
static FLUSH_DONE: Channel<CriticalSectionRawMutex, DisplayPtr, 1> = Channel::new();

/// Display ready signal. The render loop waits on this before entering
/// the lv_timer_handler loop.
pub static DISPLAY_READY: embassy_sync::signal::Signal<CriticalSectionRawMutex, ()> =
    embassy_sync::signal::Signal::new();

/// LVGL flush callback — called during lv_timer_handler when dirty areas
/// are rendered. Packages pixel data and sends to the interrupt-level
/// flush task for DMA transfer.
#[esp_hal::ram]
unsafe extern "C" fn flush_callback(
    disp: *mut lv_bevy_ecs::sys::lv_display_t,
    area_p: *const lv_bevy_ecs::sys::lv_area_t,
    px_map: *mut u8,
) {
    if disp.is_null() || area_p.is_null() || px_map.is_null() {
        return;
    }
    let area = unsafe { &*area_p };
    if area.x2 < area.x1 || area.y2 < area.y1 {
        return;
    }
    let w = (area.x2 - area.x1 + 1) as u16;
    let h = (area.y2 - area.y1 + 1) as u16;
    let Some(len_pixels) = (w as usize).checked_mul(h as usize) else {
        return;
    };
    let data_bytes = len_pixels * 2;

    let op = DrawOp {
        disp: DisplayPtr(disp),
        // SAFETY: px_map points into LVGL's static render buffer.
        // The `flushing` flag in LVGL prevents buffer reuse until
        // lv_display_flush_ready is called.
        data: unsafe { from_raw_parts(px_map, data_bytes) },
        x: area.x1 as u16,
        y: area.y1 as u16,
        w,
        h,
    };

    if DRAW.try_send(op).is_err() {
        defmt::error!("DRAW channel full — flushing inline");
        // Channel stuck — must release LVGL or it hangs in wait_callback.
        unsafe { lv_bevy_ecs::sys::lv_display_flush_ready(disp) };
        return;
    }
}

/// LVGL wait callback — blocks until the DMA transfer completes.
/// Polls the FLUSH_DONE channel in a busy-wait loop; the interrupt
/// executor's DMA completion interrupt services the sender.
///
/// If we spin ~10M iterations (~100 ms @ 240 MHz per iteration) without a
/// completion, something is wedged — call flush_ready to unstick LVGL.
#[esp_hal::ram]
unsafe extern "C" fn wait_callback(disp: *mut lv_bevy_ecs::sys::lv_display_t) {
    for _ in 0..10_000_000 {
        if let Ok(d) = FLUSH_DONE.try_receive() {
            unsafe { lv_bevy_ecs::sys::lv_display_flush_ready(d.0) };
            return;
        }
    }
    defmt::error!("flush timeout — releasing LVGL");
    // LVGL may be permanently stuck; call flush_ready to break the
    // deadlock and hope the next frame recovers.
    unsafe { lv_bevy_ecs::sys::lv_display_flush_ready(disp) };
}

/// Flush task: runs on the high-priority interrupt executor.
/// Receives pixel data from LVGL's flush callback, drains brightness
/// channel, waits for TE low (vertical blanking), writes to the CO5300
/// display via DMA, signals completion.
#[embassy_executor::task]
pub async fn flush_task(mut display: SendDisplay, mut te: SendWrap<Input<'static>>) -> ! {
    DISPLAY_READY.signal(());

    let flush_sender = FLUSH_DONE.sender();

    loop {
        trace!("F: waiting for DRAW");
        let DrawOp {
            disp: DisplayPtr(disp),
            data,
            x,
            y,
            w,
            h,
        } = DRAW.receive().await;
        trace!("F: got DRAW — write_pixels");

        // Wait for vertical blanking (TE low) to prevent tearing
        let _ = te.0.wait_for_low().await;

        let area = display_driver::Area::new(x, y, w, h);
        let fc = display_driver::bus::FrameControl::new_standalone();

        if let Err(_e) = display.0.write_pixels(area, fc, data).await {
            defmt::error!("write_pixels failed");
        }

        // Drain brightness AFTER pixel write
        if let Ok(brightness) = BRIGHTNESS_CHANNEL.try_receive() {
            if let Err(_e) = display.0.set_brightness(brightness).await {
                defmt::error!("set_brightness failed");
            }
        }

        trace!("F: sending FLUSH_DONE");
        flush_sender.send(DisplayPtr(disp)).await;
    }
}

/// Allocate full-screen DMA-aligned double buffers on the PSRAM heap.
/// Returns (buf1, buf2) pointers suitable for lv_display_set_buffers.
fn alloc_lvgl_buffers() -> (*mut c_void, *mut c_void) {
    extern crate alloc;
    use core::alloc::Layout;
    let total = BUF_BYTES * 2;
    let layout = unsafe { Layout::from_size_align_unchecked(total, 16) };
    let ptr = unsafe { alloc::alloc::alloc(layout) };
    if ptr.is_null() {
        panic!("OOM: cannot allocate LVGL full-screen buffers");
    }
    unsafe { core::ptr::write_bytes(ptr, 0, total) };
    let buf1 = ptr as *mut c_void;
    let buf2 = unsafe { ptr.add(BUF_BYTES) } as *mut c_void;
    (buf1, buf2)
}

/// Initialise LVGL display and wire up the flush pipeline.
///
/// # Safety
/// `lv_init()` must have been called once before this function.
pub unsafe fn init_lvgl_display() -> *mut lv_bevy_ecs::sys::lv_display_t {
    let (buf1, buf2) = alloc_lvgl_buffers();

    unsafe {
        let disp = lv_bevy_ecs::sys::lv_display_create(SCREEN_W, SCREEN_H);
        assert!(!disp.is_null(), "lv_display_create returned NULL");

        lv_bevy_ecs::sys::lv_display_set_color_format(
            disp,
            lv_bevy_ecs::sys::lv_color_format_t_LV_COLOR_FORMAT_RGB565_SWAPPED,
        );
        lv_bevy_ecs::sys::lv_display_set_buffers(
            disp,
            buf1,
            buf2,
            BUF_BYTES as u32,
            lv_bevy_ecs::sys::lv_display_render_mode_t_LV_DISPLAY_RENDER_MODE_FULL,
        );
        lv_bevy_ecs::sys::lv_display_set_flush_cb(disp, Some(flush_callback));
        lv_bevy_ecs::sys::lv_display_set_flush_wait_cb(disp, Some(wait_callback));

        disp
    }
}
