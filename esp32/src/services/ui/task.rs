//! LVGL render task — initialises LVGL, creates the UI, and runs the
//! `lv_timer_handler()` loop with vitals polling and theme updates.

use core::sync::atomic::Ordering;

use defmt::trace;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Receiver;
use embassy_time::{Duration, Timer};
use lv_bevy_ecs::functions::{NextTimerPeriod, lv_init, lv_tick_set_cb, lv_timer_handler};

use crate::services::rendering::display::SendDisplay;
use crate::services::rendering::task::{DISPLAY_READY, flush_task, init_lvgl_display};
use crate::services::touch;
use crate::ui::layout::{self, apply_theme};
use crate::ui::theme::CURRENT_THEME;

/// Receiver for vitals data (heart rate, SpO2, temperature) from the sensing
/// pipeline. Polled each render tick.
type VitalsReceiver = Receiver<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>;

/// Set an LVGL label to a formatted value, e.g. `set_label(handle, 72, " BPM")`
/// produces `"72 BPM"`. Uses `lv_label_set_text` directly with a stack buffer.
fn set_label_u8(handle: *mut lv_bevy_ecs::sys::lv_obj_t, val: u8, suffix: &str) {
    let mut buf = [0u8; 32];
    let suffix_bytes = suffix.as_bytes();
    let mut i = 0;

    if val >= 100 {
        buf[i] = b'0' + (val / 100);
        i += 1;
    }
    if val >= 10 {
        buf[i] = b'0' + ((val / 10) % 10);
        i += 1;
    }
    buf[i] = b'0' + (val % 10);
    i += 1;

    let remaining = buf.len().saturating_sub(i).min(suffix_bytes.len());
    buf[i..i + remaining].copy_from_slice(&suffix_bytes[..remaining]);
    i += remaining;
    buf[i] = 0;

    unsafe {
        lv_bevy_ecs::sys::lv_label_set_text(handle, buf.as_ptr() as *const core::ffi::c_char);
        lv_bevy_ecs::sys::lv_obj_invalidate(handle);
    }
}

/// Core 1 render task: initializes LVGL + display + touch, creates the tileview
/// UI, then runs the lv_timer_handler loop forever.
#[embassy_executor::task]
pub async fn render_task(
    hi_spawner: embassy_executor::SendSpawner,
    display: SendDisplay,
    mut vitals_rx: Option<VitalsReceiver>,
) -> ! {
    // 1. Init LVGL
    lv_init();
    lv_bevy_ecs::logging::connect();
    lv_tick_set_cb(|| embassy_time::Instant::now().as_millis() as u32);

    // 2. Init display and wire flush callbacks
    // SAFETY: lv_init() called above, called once.
    let _disp = unsafe { init_lvgl_display() };

    // 3. Spawn flush task on interrupt executor
    hi_spawner.spawn(flush_task(display).unwrap());

    // 4. Wait for display ready
    DISPLAY_READY.wait().await;

    // 5. Register touch indev
    let _touch = touch::register_indev();
    core::mem::forget(_touch); // lives forever

    // 6. Create UI
    let handles = layout::create_tileview();
    let mut last_theme: u8 = CURRENT_THEME.load(Ordering::Relaxed);

    // 7. Render loop
    loop {
        let start = embassy_time::Instant::now();

        trace!("R: before lv_timer_handler");
        // Drive LVGL timers (refresh, animations, indev reads)
        let period = lv_timer_handler();
        trace!("R: after lv_timer_handler");

        // Check theme toggle
        let current = CURRENT_THEME.load(Ordering::Relaxed);
        if current != last_theme {
            apply_theme(&handles);
            last_theme = current;
        }

        // Poll vitals
        if let Some(ref mut rx) = vitals_rx {
            if let Some((hr, spo2, _temp)) = rx.try_changed() {
                set_label_u8(handles.vitals.hr_label.as_ptr(), hr, " BPM");
                set_label_u8(handles.vitals.spo2_label.as_ptr(), spo2, " % SpO2");
            }
        }

        // Sleep until next timer period
        match period {
            NextTimerPeriod::Ready => {
                // At least one timer is ready — don't sleep, just yield
                Timer::after(Duration::from_millis(5)).await;
            }
            NextTimerPeriod::AfterMs(ms) => {
                let elapsed = start.elapsed().as_millis() as u32;
                let sleep_ms = ms.get().saturating_sub(elapsed).max(1);
                Timer::after(Duration::from_millis(sleep_ms.into())).await;
            }
            NextTimerPeriod::Never => {
                Timer::after(Duration::from_secs(1)).await;
            }
        }
    }
}
