//! LVGL display init, async DMA flush pipeline, and render loop.
//!
//! The flush pipeline:
//!   flush_callback (LVGL render thread) → DRAW channel →
//!   flush_task (interrupt executor) → DMA transfer → FLUSH_DONE →
//!   wait_callback → returns (LVGL clears flushing flag)
//!
//! Uses FULL render mode with two full-resolution PSRAM draw buffers
//! (466×466×2 = 434 KB each). LVGL renders the entire screen each frame;
//! the flush callback DMAs the rendered buffer to the CO5300 display via
//! windowed QSPI. Double buffering lets LVGL render one frame while
//! the previous frame's DMA transfer is still in flight.

use core::ffi::c_void;
use core::slice::from_raw_parts;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering;

use defmt::trace;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Instant;
use esp_hal::gpio::Input;

use crate::services::rendering::display::SendDisplay;
use crate::ui::config::PRODUCTION_UI_SIZE;
use crate::utils::{PerfCounter, SendWrap};

/// LVGL screen dimensions in logical pixels (matches physical display).
pub const SCREEN_W: i32 = PRODUCTION_UI_SIZE as i32;
pub const SCREEN_H: i32 = PRODUCTION_UI_SIZE as i32;

/// Draw buffer byte size (466×466×2 for RGB565) for one full-res buffer.
pub const BUF_BYTES: usize = PRODUCTION_UI_SIZE as usize * PRODUCTION_UI_SIZE as usize * 2;

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
    frame_id: u32,
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

// ── Frame performance counters ─────────────────────────────────────────

/// Named performance counter instance for frame-timing data.
static PERF: PerfCounter<8> = PerfCounter::new();

/// Set frame start timestamp (call from UI task before `lv_timer_handler`).
pub fn mark_frame_start() {
    PERF.start("frame", Instant::now());
    PERF.start("lvgl", Instant::now());
}

/// Last frame we logged (for stale-detection).
static LAST_LOGGED_FRAME: AtomicU32 = AtomicU32::new(0);

/// Log a one-line perf summary after each frame.
/// Call from the UI task after `lv_timer_handler` returns.
/// Skips logging when no render occurred (flush_callback wasn't called).
/// DMA/TE counters are only valid when dma_frame_id == frame_id.
pub fn log_perf() {
    let now = Instant::now();

    let fid = PERF.get("frame_id");
    if fid == 0 || fid == LAST_LOGGED_FRAME.load(Ordering::Relaxed) {
        return;
    }
    LAST_LOGGED_FRAME.store(fid, Ordering::Relaxed);

    PERF.stop("frame", now);
    let total = PERF.get("frame");
    let lvgl = PERF.get("lvgl");
    let dfid = PERF.get("dma_frame_id");

    let (te_wait, dma) = if dfid == fid {
        (PERF.get("te_wait"), PERF.get("dma"))
    } else {
        (0, 0)
    };
    if dfid == fid {
        defmt::trace!(
            "PERF  lvgl={}μs  te_wait={}μs  dma={}μs  total={}μs  fid={}",
            lvgl,
            te_wait,
            dma,
            total,
            fid
        );
    } else {
        defmt::trace!(
            "PERF  lvgl={}μs  dma=SKIP  total={}μs  fid={}",
            lvgl,
            total,
            fid
        );
    }
}

// ── Flush callback ─────────────────────────────────────────────────────

/// LVGL flush callback — called once per frame in FULL mode.
///
/// `px_map` points to the start of the full-resolution draw buffer and the
/// area covers the whole display. The 2-pixel alignment expansion is a
/// no-op for full-screen flushes but is kept for safety with CO5300.
/// We send the raw pixel data to the DMA task, which writes it to the
/// display via a windowed QSPI command.
#[esp_hal::ram(unstable(rtc_fast))]
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

    // ── Perf: LVGL done, frame_id ──
    PERF.stop("lvgl", Instant::now());
    let fid = PERF.get("frame_id").wrapping_add(1);
    PERF.set("frame_id", fid);

    // CO5300 requires 2-pixel alignment (x, y, w, h must all be even).
    // With FULL mode the area always covers the entire screen so this is a
    // no-op, but we keep the alignment for safety.
    let x1 = area.x1 & !1;
    let y1 = area.y1 & !1;
    let x2 = area.x2 | 1;
    let y2 = area.y2 | 1;

    let w = (x2 - x1 + 1) as u16;
    let h = (y2 - y1 + 1) as u16;
    let bytes = w as usize * h as usize * 2;

    // Adjust px_map to point to the aligned top-left (x1, y1) instead of
    // the original (area.x1, area.y1). Safe because the frame buffer is one
    // contiguous allocation and we only expand within screen bounds.
    let row_stride_bytes = SCREEN_W as usize * 2;
    let offset = ((area.x1 - x1) as usize) * 2 + ((area.y1 - y1) as usize) * row_stride_bytes;
    let data = unsafe { from_raw_parts(px_map.sub(offset), bytes) };

    let op = DrawOp {
        disp: DisplayPtr(disp),
        data,
        x: x1 as u16,
        y: y1 as u16,
        w,
        h,
        frame_id: fid,
    };

    if DRAW.try_send(op).is_err() {
        defmt::error!("DRAW channel full — flushing inline");
        unsafe { lv_bevy_ecs::sys::lv_display_flush_ready(disp) };
    }
}

// ── Wait callback ──────────────────────────────────────────────────────

/// LVGL wait callback — blocks until the DMA transfer completes.
/// Polls the FLUSH_DONE channel in a busy-wait loop; the interrupt
/// executor's DMA completion interrupt services the sender.
///
/// If we spin ~10M iterations (~100 ms @ 240 MHz per iteration) without a
/// completion, something is wedged — call flush_ready to unstick LVGL.
#[esp_hal::ram(unstable(rtc_fast))]
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

// ── Flush task ─────────────────────────────────────────────────────────

/// Flush task: runs on the high-priority interrupt executor.
/// Receives full-res pixel data from LVGL's flush callback, drains
/// brightness channel, waits for TE low (vertical blanking), writes to
/// the CO5300 display via DMA, signals completion.
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
            frame_id,
        } = DRAW.receive().await;
        trace!("F: got DRAW — write_pixels");

        PERF.set("dma_frame_id", frame_id);

        // ── Perf: TE wait ──
        PERF.start("te_wait", Instant::now());
        let _ = te.0.wait_for_low().await;
        PERF.stop("te_wait", Instant::now());

        // ── Perf: DMA transfer ──
        PERF.start("dma", Instant::now());

        let area = display_driver::Area::new(x, y, w, h);
        let fc = display_driver::bus::FrameControl::new_standalone();

        if let Err(_e) = display.0.write_pixels(area, fc, data).await {
            defmt::error!("write_pixels failed");
        }

        PERF.stop("dma", Instant::now());

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

// ── Buffer allocation ──────────────────────────────────────────────────

/// Allocate two full-resolution PSRAM draw buffers for DIRECT double
/// buffering. LVGL renders into one while the DMA task flushes the other.
fn alloc_lvgl_buffers() -> (*mut c_void, *mut c_void) {
    extern crate alloc;
    use core::alloc::Layout;

    let layout = unsafe { Layout::from_size_align_unchecked(BUF_BYTES, 16) };
    let buf1 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    let buf2 = unsafe { alloc::alloc::alloc_zeroed(layout) };
    if buf1.is_null() || buf2.is_null() {
        panic!("OOM: cannot allocate LVGL draw buffers");
    }
    (buf1 as *mut c_void, buf2 as *mut c_void)
}

// ── Display init ───────────────────────────────────────────────────────

/// Initialise LVGL display at full resolution (466×466) with FULL
/// render mode double buffering.
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
