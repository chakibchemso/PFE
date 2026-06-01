use core::sync::atomic::Ordering;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Receiver;
use oxivgl::display::LvglBuffers;
use oxivgl::view::{NavAction, View};
use oxivgl::widgets::{Obj, WidgetError};

use super::super::rendering::task::{LVGL_BUF_BYTES, SCREEN_H, SCREEN_W};
use crate::services::touch;
use crate::ui::layout::{AppHandles, apply_theme, create_tileview};
use crate::ui::theme::CURRENT_THEME;

/// Receiver for vitals data (heart rate, SpO2, temperature) from the sensing
/// pipeline. Polled in [`WatchView::update`] each render tick.
type VitalsReceiver = Receiver<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>;

pub struct WatchView {
    tileview: Option<oxivgl::widgets::Tileview<'static>>,
    handles: Option<AppHandles>,
    indev_registered: bool,
    last_theme: u8,
    vitals_rx: Option<VitalsReceiver>,
}

impl WatchView {
    pub fn new(vitals_rx: Option<VitalsReceiver>) -> Self {
        Self {
            tileview: None,
            handles: None,
            indev_registered: false,
            last_theme: 1,
            vitals_rx,
        }
    }
}

impl Default for WatchView {
    fn default() -> Self {
        Self::new(None)
    }
}

impl View for WatchView {
    fn create(&mut self, container: &Obj<'static>) -> Result<(), WidgetError> {
        if !self.indev_registered {
            touch::register_indev();
            self.indev_registered = true;
        }

        let (handles, tileview) = create_tileview(container)?;
        self.tileview = Some(tileview);
        apply_theme(&handles);
        self.handles = Some(handles);
        self.last_theme = CURRENT_THEME.load(Ordering::Relaxed);

        Ok(())
    }

    fn update(&mut self) -> Result<NavAction, WidgetError> {
        let current = CURRENT_THEME.load(Ordering::Relaxed);
        if current != self.last_theme {
            if let Some(ref handles) = self.handles {
                apply_theme(handles);
            }
            self.last_theme = current;
        }

        if let Some(ref mut rx) = self.vitals_rx {
            if let Some((hr, spo2, _temp)) = rx.try_changed() {
                if let Some(ref handles) = self.handles {
                    set_label_u8(handles.hr_label, hr, " BPM");
                    set_label_u8(handles.spo2_label, spo2, " % SpO2");
                }
            }
        }

        Ok(NavAction::None)
    }
}

/// Set an LVGL label to a formatted value, e.g. `set_label(handle, 72, " BPM")`
/// produces `"72 BPM"`.
fn set_label_u8(handle: *mut oxivgl_sys::lv_obj_t, val: u8, suffix: &str) {
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
        oxivgl_sys::lv_label_set_text(handle, buf.as_ptr() as *const core::ffi::c_char);
        // Explicit invalidation — lv_label_set_text should already call
        // lv_obj_invalidate internally, but on embedded targets with FULL
        // render mode we've seen cases where the invalidation doesn't
        // propagate correctly without an explicit call.
        oxivgl_sys::lv_obj_invalidate(handle);
    }
}

#[embassy_executor::task]
pub async fn render_task(
    bufs: &'static mut LvglBuffers<LVGL_BUF_BYTES>,
    vitals_rx: Option<VitalsReceiver>,
) -> ! {
    let view = WatchView::new(vitals_rx);
    oxivgl::view::run_app::<WatchView, LVGL_BUF_BYTES>(SCREEN_W, SCREEN_H, bufs, view).await
}
