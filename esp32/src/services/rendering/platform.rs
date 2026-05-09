use alloc::boxed::Box;
use alloc::rc::Rc;
use embassy_time::Instant;
use slint::platform::{
    Platform, WindowAdapter,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};

/// Slint platform adapter for ESP32
struct EspPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl Platform for EspPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(Instant::now().as_millis())
    }
}

/// Initialize the Slint platform and return the main window
pub fn init_platform(viewport_size: u16) -> Rc<MinimalSoftwareWindow> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);

    slint::platform::set_platform(Box::new(EspPlatform {
        window: window.clone(),
    }))
    .expect("Failed to set Slint platform");

    window.set_size(slint::PhysicalSize::new(
        viewport_size as u32,
        viewport_size as u32,
    ));

    window
}
