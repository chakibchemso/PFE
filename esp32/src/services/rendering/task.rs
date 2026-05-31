use core::alloc::GlobalAlloc;
use core::sync::atomic::{AtomicU32, Ordering};
use defmt::info;
use oxivgl::{
    display::LvglBuffers,
    enums::EventCode,
    event::Event,
    view::{NavAction, View},
    widgets::{Align, Button, Child, Label, Obj, Slider, WidgetError},
};

use crate::services::rendering::display::OxivglDisplay;
use crate::services::touch;
use crate::ui::config::PRODUCTION_UI_SIZE;

/// Screen dimensions in LVGL pixel coordinates.
pub const SCREEN_W: i32 = PRODUCTION_UI_SIZE as i32;
pub const SCREEN_H: i32 = PRODUCTION_UI_SIZE as i32;

/// Buffer size: full screen × 2 bytes/pixel (RGB565).
pub const LVGL_BUF_BYTES: usize = SCREEN_W as usize * SCREEN_H as usize * 2;

/// Flush task: runs on interrupt executor, receives pixel stripes from LVGL
/// and writes them to the CO5300 display via DMA.
#[embassy_executor::task]
pub async fn flush_task(display: OxivglDisplay) -> ! {
    oxivgl::flush_pipeline::flush_frame_buffer(display).await
}

/// Allocate full-screen LVGL double-buffers on the PSRAM heap.
///
/// Must be called once, then the reference passed to [`render_task`].
pub fn take_lvgl_buffers() -> &'static mut LvglBuffers<LVGL_BUF_BYTES> {
    use core::alloc::Layout;
    // LvglBuf is `#[repr(align(16))]`, compiler pads size to a 16-byte boundary.
    let buf_size = (LVGL_BUF_BYTES + 15) & !15;
    let total = buf_size * 2;
    let layout = unsafe { Layout::from_size_align_unchecked(total, 16) };
    let ptr = unsafe { esp_alloc::HEAP.alloc(layout) };
    if ptr.is_null() {
        panic!("OOM: cannot allocate LVGL full-screen buffers");
    }
    // Zero-initialise (same as LvglBuffers::new()).
    unsafe { core::ptr::write_bytes(ptr, 0, total) };
    // SAFETY: called once, single-threaded on core 1.
    unsafe { &mut *(ptr as *mut LvglBuffers<LVGL_BUF_BYTES>) }
}

/// LVGL render task: runs on thread-mode executor. Initialises LVGL, creates the
/// view, and drives the render/timer loop forever.
#[embassy_executor::task]
pub async fn render_task(bufs: &'static mut LvglBuffers<LVGL_BUF_BYTES>) -> ! {
    let view = HelloView::new();
    info!("UI task starting");
    oxivgl::view::run_app::<HelloView, LVGL_BUF_BYTES>(SCREEN_W, SCREEN_H, bufs, view).await
}

// ── Shared LVGL event callbacks ────────────────────────────────────────────

static BUTTON_CLICKS: AtomicU32 = AtomicU32::new(0);

fn on_button_clicked(_ev: &Event) {
    let n = BUTTON_CLICKS.fetch_add(1, Ordering::Relaxed) + 1;
    info!("Button clicked! count={}", n);
}

fn on_slider_changed(ev: &Event) {
    let handle = ev.current_target_handle();
    let val = unsafe { oxivgl_sys::lv_slider_get_value(handle) };
    info!("Slider value: {}", val);
}

// ── Hello LVGL View ────────────────────────────────────────────────────────

/// Touch test view — button, slider, and background toggle.
pub struct HelloView {
    _btn_label: Option<Child<Label<'static>>>,
    counter_label: Option<Child<Label<'static>>>,
    value_label: Option<Child<Label<'static>>>,
    _button: Option<Child<Button<'static>>>,
    _slider: Option<Child<Slider<'static>>>,
    screen_handle: *mut oxivgl_sys::lv_obj_t,
    indev_registered: bool,
    bg_toggle: bool,
    was_pressed: bool,
    last_click_count: u32,
    last_slider_val: i32,
}

impl HelloView {
    pub fn new() -> Self {
        Self {
            _btn_label: None,
            counter_label: None,
            value_label: None,
            _button: None,
            _slider: None,
            screen_handle: core::ptr::null_mut(),
            indev_registered: false,
            bg_toggle: false,
            was_pressed: false,
            last_click_count: 0,
            last_slider_val: 50,
        }
    }
}

impl View for HelloView {
    fn create(&mut self, container: &Obj<'static>) -> Result<(), WidgetError> {
        info!("HelloView::create");

        if !self.indev_registered {
            touch::register_indev();
            self.indev_registered = true;
        }

        self.screen_handle = container.handle();
        self.bg_toggle = false;
        self.was_pressed = false;
        container.bg_color(0x003a57).bg_opa(255);
        container.text_color(0xffffff);

        // ── Button ──────────────────────────────────────────────────────
        let btn = Button::new(container)?;
        btn.size(160, 48).bg_color(0x0078d7).align(Align::Center, 0, -60);
        let btn_label = Label::new(&btn)?;
        btn_label.text("Tap me!").align(Align::Center, 0, 0);
        self._btn_label = Some(Child::new(btn_label));
        btn.on(EventCode::CLICKED, on_button_clicked);
        self._button = Some(Child::new(btn));

        // ── Click counter ───────────────────────────────────────────────
        let clabel = Label::new(container)?;
        clabel.text("Clicks: 0").align(Align::Center, 0, -20);
        self.counter_label = Some(Child::new(clabel));

        // ── Slider ──────────────────────────────────────────────────────
        let slider = Slider::new(container)?;
        slider.set_range(0, 100).set_value(50);
        slider.size(250, 20).align(Align::Center, 0, 40);
        slider.on(EventCode::VALUE_CHANGED, on_slider_changed);
        self._slider = Some(Child::new(slider));

        // ── Slider value display ────────────────────────────────────────
        let vlabel = Label::new(container)?;
        vlabel.text("Slider: 50").align(Align::Center, 0, 75);
        self.value_label = Some(Child::new(vlabel));

        Ok(())
    }

    fn update(&mut self) -> Result<NavAction, WidgetError> {
        let pressed = touch::TOUCH_PRESSED.load(Ordering::Relaxed);

        // Toggle background on touch release (anywhere on screen).
        if self.was_pressed && !pressed {
            let x = touch::TOUCH_X.load(Ordering::Relaxed);
            let y = touch::TOUCH_Y.load(Ordering::Relaxed);
            info!("Touch released: ({}, {})", x, y);
            self.bg_toggle = !self.bg_toggle;

            let screen = Obj::from_raw_non_owning(self.screen_handle);
            if self.bg_toggle {
                screen.bg_color(0x571600).bg_opa(255);
            } else {
                screen.bg_color(0x003a57).bg_opa(255);
            }
        }
        self.was_pressed = pressed;

        // Refresh button click counter label.
        let count = BUTTON_CLICKS.load(Ordering::Relaxed);
        if count != self.last_click_count {
            self.last_click_count = count;
            if let Some(ref label) = self.counter_label {
                label.text(&alloc::format!("Clicks: {}", count));
            }
        }

        // Refresh slider value label.
        if let Some(ref slider) = self._slider {
            let val = slider.get_value();
            if val != self.last_slider_val {
                self.last_slider_val = val;
                if let Some(ref label) = self.value_label {
                    label.text(&alloc::format!("Slider: {}", val));
                }
            }
        }

        Ok(NavAction::None)
    }
}
