//! LVGL render task — initialises LVGL, creates the UI, and runs the
//! `lv_timer_handler()` loop with vitals polling and theme updates.

use alloc::vec::Vec;
use core::ffi::CStr;
use core::sync::atomic::Ordering;

use defmt::trace;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::watch::Receiver;
use embassy_time::{Duration, Timer};
use lv_bevy_ecs::cstr;
use lv_bevy_ecs::functions::{NextTimerPeriod, lv_init, lv_tick_set_cb, lv_timer_handler};

use embassy_executor::Spawner;
use esp_hal::gpio::Input;

use crate::utils::SendWrap;

/// Channel for settings saves from UI callbacks (non-async → async bridge).
/// Keyboard/GMT overlays send pending saves here; the render loop processes them.
#[derive(Clone)]
pub struct PendingSave {
    pub key: &'static str,
    pub data: Vec<u8>,
}

pub static PENDING_SAVES: Channel<CriticalSectionRawMutex, PendingSave, 8> = Channel::new();

use crate::app::bus::BatteryState;
use crate::drivers::oxymeter::WAVEFORM_CHANNEL;
use crate::services::rendering::display::SendDisplay;
use crate::services::rendering::task::{
    BRIGHTNESS_CHANNEL, DISPLAY_READY, flush_task, init_lvgl_display,
};
use crate::services::storage::StorageService;
use crate::services::storage::StoredConfig;
use crate::services::touch;
use crate::ui::gmt::LAST_GMT_OFFSET;
use crate::ui::layout::{self, apply_theme};
use crate::ui::theme::CURRENT_BRIGHTNESS;
use crate::ui::theme::CURRENT_THEME;

/// Receiver for vitals data (heart rate, SpO2, temperature) from the sensing
/// pipeline. Polled each render tick.
type VitalsReceiver = Receiver<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>;

/// Receiver for wifi connection state.
type WifiReceiver = Receiver<'static, CriticalSectionRawMutex, bool, 2>;

/// Receiver for MQTT broker connection state.
type MqttReceiver = Receiver<'static, CriticalSectionRawMutex, bool, 2>;

/// Receiver for UTC epoch from NTP sync.
type UtcReceiver = Receiver<'static, CriticalSectionRawMutex, u64, 2>;

/// Receiver for battery state.
type BatteryReceiver = Receiver<'static, CriticalSectionRawMutex, BatteryState, 2>;

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
    _spawner: Spawner,
    hi_spawner: embassy_executor::SendSpawner,
    display: SendDisplay,
    te: Input<'static>,
    mut vitals_rx: Option<VitalsReceiver>,
    mut wifi_rx: Option<WifiReceiver>,
    mut mqtt_rx: Option<MqttReceiver>,
    utc_rx: Option<UtcReceiver>,
    mut battery_rx: Option<BatteryReceiver>,
    storage: &'static StorageService,
    stored_config: &'static Mutex<CriticalSectionRawMutex, StoredConfig>,
) -> ! {
    // 1. Init LVGL
    lv_init();
    lv_bevy_ecs::logging::connect();
    lv_tick_set_cb(|| embassy_time::Instant::now().as_millis() as u32);

    // 2. Init display and wire flush callbacks
    // SAFETY: lv_init() called above, called once.
    let _disp = unsafe { init_lvgl_display() };

    // 3. Spawn flush task on interrupt executor
    hi_spawner.spawn(flush_task(display, SendWrap(te)).unwrap());

    // 4. Wait for display ready
    DISPLAY_READY.wait().await;

    // 5. Register touch indev
    let _touch = touch::register_indev();
    core::mem::forget(_touch); // lives forever

    // 6. Seed runtime state from stored config
    {
        let cfg = stored_config.lock().await;
        LAST_GMT_OFFSET.store(cfg.gmt_offset, Ordering::Relaxed);
        CURRENT_THEME.store(cfg.theme, Ordering::Relaxed);
        CURRENT_BRIGHTNESS.store(cfg.brightness, Ordering::Relaxed);
        let hw = ((cfg.brightness as u16 * 255 + 50) / 100).min(255) as u8;
        let _ = BRIGHTNESS_CHANNEL.try_send(hw);
    }

    // 7. Create UI
    let handles = layout::create_tileview();
    let mut last_theme: u8 = CURRENT_THEME.load(Ordering::Relaxed);

    // BPM ring buffer for min-max range display
    let mut bpm_buf = [0u8; 10];
    let mut bpm_idx = 0usize;
    let mut bpm_count = 0usize;

    // Watch tick state (inline, no separate task to avoid async invalidation)
    let mut last_tick = embassy_time::Instant::now();
    let mut utc_rx = utc_rx;
    let mut epoch_secs = 0u64;

    // 8. Render loop
    loop {
        let start = embassy_time::Instant::now();

        // ── Watch face tick (before timer handler so invalidation is batched) ──
        let now = embassy_time::Instant::now();
        if now - last_tick >= embassy_time::Duration::from_millis(980) {
            last_tick = now;

            if let Some(ref mut rx) = utc_rx {
                if let Some(new_epoch) = rx.try_changed() {
                    epoch_secs = new_epoch;
                } else {
                    epoch_secs = epoch_secs.wrapping_add(1);
                }
            } else {
                epoch_secs = epoch_secs.wrapping_add(1);
            }

            // Apply GMT offset from stored config
            let gmt_offset = {
                let cfg = stored_config.lock().await;
                cfg.gmt_offset as i64
            };
            let local = ((epoch_secs as i64) + gmt_offset * 3600).rem_euclid(86400) as u64;
            let hours = local / 3600;
            let mins = (local / 60) % 60;
            let secs = local % 60;

            let h_rot = ((hours % 12) * 300 + mins * 5) as i32;
            let m_rot = (mins * 60 + secs) as i32;
            let s_rot = (secs * 60) as i32;

            let mut buf = [0u8; 9];
            buf[0] = b'0' + (hours / 10) as u8;
            buf[1] = b'0' + (hours % 10) as u8;
            buf[2] = b':';
            buf[3] = b'0' + (mins / 10) as u8;
            buf[4] = b'0' + (mins % 10) as u8;
            buf[5] = b':';
            buf[6] = b'0' + (secs / 10) as u8;
            buf[7] = b'0' + (secs % 10) as u8;
            buf[8] = 0;

            unsafe {
                lv_bevy_ecs::sys::lv_obj_set_style_transform_rotation(
                    handles.watchface.hour_hand,
                    h_rot,
                    0,
                );
                lv_bevy_ecs::sys::lv_obj_set_style_transform_rotation(
                    handles.watchface.minute_hand,
                    m_rot,
                    0,
                );
                lv_bevy_ecs::sys::lv_obj_set_style_transform_rotation(
                    handles.watchface.second_hand,
                    s_rot,
                    0,
                );
                lv_bevy_ecs::sys::lv_label_set_text(
                    handles.watchface.digital_time,
                    buf.as_ptr() as *const core::ffi::c_char,
                );
            }
        }

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

        // Process pending settings saves (from keyboard/GMT overlays)
        while let Ok(save) = PENDING_SAVES.try_receive() {
            storage.write(save.key, &save.data).await;
            let mut cfg = stored_config.lock().await;
            match save.key {
                crate::services::storage::KEY_WIFI_SSID => {
                    cfg.wifi_ssid = save.data;
                }
                crate::services::storage::KEY_WIFI_PASSWD => {
                    cfg.wifi_passwd = save.data;
                }
                crate::services::storage::KEY_ASCON => {
                    if save.data.len() == 16 {
                        cfg.ascon_key.copy_from_slice(&save.data);
                    }
                }
                crate::services::storage::KEY_GMT_OFFSET => {
                    if let Some(&b) = save.data.first() {
                        let off = b as i8;
                        cfg.gmt_offset = off;
                        LAST_GMT_OFFSET.store(off, Ordering::Relaxed);
                    }
                }
                crate::services::storage::KEY_THEME => {
                    if let Some(&t) = save.data.first() {
                        cfg.theme = t;
                        CURRENT_THEME.store(t, Ordering::Relaxed);
                    }
                }
                crate::services::storage::KEY_BRIGHTNESS => {
                    if let Some(&b) = save.data.first() {
                        cfg.brightness = b;
                        CURRENT_BRIGHTNESS.store(b, Ordering::Relaxed);
                        let hw = ((b as u16 * 255 + 50) / 100).min(255) as u8;
                        let _ = BRIGHTNESS_CHANNEL.try_send(hw);
                    }
                }
                _ => {}
            }
        }

        // Poll vitals
        if let Some(ref mut rx) = vitals_rx {
            if let Some((hr, spo2, temp)) = rx.try_changed() {
                if hr == 0 && spo2 == 0 {
                    // No finger on sensor — show placeholder + reset BPM buffer
                    bpm_count = 0;
                    bpm_idx = 0;
                    unsafe {
                        lv_bevy_ecs::sys::lv_label_set_text(
                            handles.vitals.bpm_label,
                            cstr!("-- BPM").as_ptr(),
                        );
                        lv_bevy_ecs::sys::lv_bar_set_start_value(
                            handles.vitals.bpm_range_bar,
                            0,
                            false,
                        );
                        lv_bevy_ecs::sys::lv_bar_set_value(handles.vitals.bpm_range_bar, 0, false);
                        lv_bevy_ecs::sys::lv_slider_set_value(handles.vitals.bpm_slider, 10, false);
                        lv_bevy_ecs::sys::lv_label_set_text(
                            handles.vitals.spo2_label,
                            cstr!("SpO2 --%").as_ptr(),
                        );
                        lv_bevy_ecs::sys::lv_bar_set_value(handles.vitals.spo2_bar, 0, false);
                        lv_bevy_ecs::sys::lv_label_set_text(
                            handles.vitals.temp_label,
                            cstr!("--°C").as_ptr(),
                        );
                    }
                } else {
                    // BPM ring buffer
                    bpm_buf[bpm_idx] = hr;
                    bpm_idx = (bpm_idx + 1) % 10;
                    bpm_count = core::cmp::min(bpm_count + 1, 10);
                    let min = *bpm_buf[..bpm_count].iter().min().unwrap();
                    let max = *bpm_buf[..bpm_count].iter().max().unwrap();

                    unsafe {
                        // BPM label + range bar + slider
                        set_label_u8(handles.vitals.bpm_label, hr, " BPM");
                        lv_bevy_ecs::sys::lv_bar_set_start_value(
                            handles.vitals.bpm_range_bar,
                            min as i32,
                            false,
                        );
                        lv_bevy_ecs::sys::lv_bar_set_value(
                            handles.vitals.bpm_range_bar,
                            max as i32,
                            false,
                        );
                        lv_bevy_ecs::sys::lv_slider_set_value(
                            handles.vitals.bpm_slider,
                            (hr as i32).clamp(10, 190),
                            false,
                        );

                        // SpO₂ label + bar with color threshold
                        set_label_u8(handles.vitals.spo2_label, spo2, "% SpO2");
                        let pal = crate::ui::theme::current_palette();
                        let spo2_color = if spo2 < 80 {
                            pal.unhealthy_color
                        } else {
                            pal.healthy_color
                        };
                        lv_bevy_ecs::sys::lv_obj_set_style_bg_color(
                            handles.vitals.spo2_bar,
                            lv_bevy_ecs::functions::lv_color_hex(spo2_color),
                            lv_bevy_ecs::sys::lv_part_t_LV_PART_INDICATOR,
                        );
                        lv_bevy_ecs::sys::lv_bar_set_value(
                            handles.vitals.spo2_bar,
                            spo2 as i32,
                            false,
                        );

                        // Temperature
                        set_label_u8(handles.vitals.temp_label, temp, "°C");
                    }
                }
            }
        }

        // Drain waveform samples into PPG chart
        while let Ok((red, ir)) = WAVEFORM_CHANNEL.try_receive() {
            unsafe {
                lv_bevy_ecs::sys::lv_chart_set_next_value(
                    handles.vitals.chart,
                    handles.vitals.red_series,
                    red as i32,
                );
                lv_bevy_ecs::sys::lv_chart_set_next_value(
                    handles.vitals.chart,
                    handles.vitals.ir_series,
                    ir as i32,
                );
            }
        }

        // Poll wifi status
        if let Some(ref mut rx) = wifi_rx {
            if let Some(connected) = rx.try_changed() {
                unsafe {
                    if connected {
                        lv_bevy_ecs::sys::lv_led_set_color(
                            handles.watchface.wifi_led,
                            lv_bevy_ecs::functions::lv_color_hex(0xa6e3a1),
                        );
                        lv_bevy_ecs::sys::lv_led_on(handles.watchface.wifi_led);
                    } else {
                        lv_bevy_ecs::sys::lv_led_off(handles.watchface.wifi_led);
                    }
                }
            }
        }

        // Poll MQTT status
        if let Some(ref mut rx) = mqtt_rx {
            if let Some(connected) = rx.try_changed() {
                unsafe {
                    if connected {
                        lv_bevy_ecs::sys::lv_led_set_color(
                            handles.watchface.mqtt_led,
                            lv_bevy_ecs::functions::lv_color_hex(0xa6e3a1),
                        );
                        lv_bevy_ecs::sys::lv_led_on(handles.watchface.mqtt_led);
                    } else {
                        lv_bevy_ecs::sys::lv_led_off(handles.watchface.mqtt_led);
                    }
                }
            }
        }

        // Poll battery status
        if let Some(ref mut rx) = battery_rx {
            if let Some(state) = rx.try_changed() {
                unsafe {
                    if state.charging {
                        lv_bevy_ecs::sys::lv_obj_set_style_bg_color(
                            handles.watchface.battery_bar,
                            lv_bevy_ecs::functions::lv_color_hex(0xa6e3a1),
                            lv_bevy_ecs::sys::lv_part_t_LV_PART_INDICATOR,
                        );
                    } else {
                        let pal = crate::ui::theme::current_palette();
                        lv_bevy_ecs::sys::lv_obj_set_style_bg_color(
                            handles.watchface.battery_bar,
                            lv_bevy_ecs::functions::lv_color_hex(pal.accent_color),
                            lv_bevy_ecs::sys::lv_part_t_LV_PART_INDICATOR,
                        );
                    }

                    match state.pct {
                        Some(pct) => {
                            lv_bevy_ecs::sys::lv_bar_set_value(
                                handles.watchface.battery_bar,
                                pct as i32,
                                false,
                            );
                            set_label_u8(handles.watchface.battery_pct, pct, "%");
                        }
                        None => {
                            lv_bevy_ecs::sys::lv_bar_set_value(
                                handles.watchface.battery_bar,
                                0,
                                false,
                            );
                            lv_bevy_ecs::sys::lv_label_set_text(
                                handles.watchface.battery_pct,
                                cstr!("NA").as_ptr(),
                            );
                        }
                    }
                }
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
