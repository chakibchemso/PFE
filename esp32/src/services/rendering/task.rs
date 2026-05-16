use alloc::rc::Rc;
use core::cell::RefCell;
use defmt::info;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::watch::Receiver;
use embassy_time::{Duration, Instant, Timer};
use slint::ComponentHandle;
use slint::platform::software_renderer::MinimalSoftwareWindow;

use crate::app::bus::GpsFix;
use crate::ui::SmartWatchUI;
use crate::ui::config::RenderConfig;
use crate::ui::widgets::gps::GpsState;
use crate::ui::widgets::watchface;

use super::display::SmartWatchDisplay;
use super::framebuffer::Framebuffer;

/// Shared window handle for the touch task to dispatch events
pub type SharedWindow = Mutex<CriticalSectionRawMutex, RefCell<Option<Rc<MinimalSoftwareWindow>>>>;

/// Main render loop: initializes Slint UI, reads bus channels, renders frames.
#[embassy_executor::task]
pub async fn render_task(
    config: RenderConfig,
    mut display: SmartWatchDisplay,
    shared_window: &'static SharedWindow,
    window: Rc<MinimalSoftwareWindow>,
    mut vitals_receiver: Receiver<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>,
    mut wifi_receiver: Receiver<'static, CriticalSectionRawMutex, bool, 2>,
    mut gps_receiver: Receiver<'static, CriticalSectionRawMutex, Option<GpsFix>, 2>,
    mut cpu_temp_receiver: Receiver<'static, CriticalSectionRawMutex, i8, 2>,
) {
    let viewport_size = config.viewport_size as usize;
    let mut fb = Framebuffer::new(viewport_size);

    let ui = SmartWatchUI::new().unwrap();
    ui.set_time(slint::format!("12:00"));
    ui.set_bpm(slint::format!("--"));
    ui.set_spo2(slint::format!("--"));
    ui.set_hour_angle(0.0);
    ui.set_minute_angle(0.0);
    ui.set_wifi_connected(false);
    ui.set_ascon_secure(true);
    ui.set_gps_fix(false);
    ui.set_dark_mode(true);
    ui.set_show_fps(false);
    ui.set_cpu_temp(slint::format!("--°C"));
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
        let frame_start = Instant::now();

        // Update WiFi status
        if let Some(connected) = wifi_receiver.try_changed() {
            ui.set_wifi_connected(connected);
            window.request_redraw();
        }

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
            ui.set_bpm(slint::format!("{}", bpm));
            ui.set_spo2(slint::format!("{}", spo2));
            window.request_redraw();
        }

        // Update CPU die temperature
        if let Some(temp) = cpu_temp_receiver.try_changed() {
            ui.set_cpu_temp(slint::format!("{}°C", temp));
            window.request_redraw();
        }

        // Update GPS
        if let Some(gps_fix) = gps_receiver.try_changed() {
            let state = match gps_fix {
                Some(ref fix) => GpsState::from_fix(fix),
                None => GpsState::searching(),
            };
            ui.set_gps_fix(state.has_fix);
            ui.set_gps_heading(state.heading);
            ui.set_gps_coords(state.coords);
            ui.set_gps_speed(state.speed);
            ui.set_gps_sats(state.sats);
            ui.set_gps_altitude(state.altitude);
            window.request_redraw();
        }

        slint::platform::update_timers_and_animations();

        let drew_anything = fb.render(&window, viewport_size);

        if drew_anything {
            let _transfer_us = fb.transfer_full(config, &mut display).await;
        }

        // Cap at 30 FPS (33ms period)
        let elapsed = frame_start.elapsed();
        info!("elapsed: {}", elapsed.as_millis());
        if elapsed < Duration::from_millis(100) {
            Timer::after(Duration::from_millis(100) - elapsed).await;
        }
    }
}
