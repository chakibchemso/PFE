use alloc::rc::Rc;
use embassy_executor::Spawner;
use slint::platform::software_renderer::MinimalSoftwareWindow;

use crate::app::bus::SystemBus;
use crate::ui::config::RenderConfig;

pub mod display;
pub mod framebuffer;
pub mod platform;
pub mod task;

pub use display::SmartWatchDisplay;
pub use task::SharedWindow;

/// Spawn the rendering service: Slint platform + display init + main render loop.
pub fn register(
    spawner: &Spawner,
    config: RenderConfig,
    display: SmartWatchDisplay,
    shared_window: &'static SharedWindow,
    window: Rc<MinimalSoftwareWindow>,
    bus: &'static SystemBus,
) {
    let vitals_rx = bus.vitals.receiver().expect("vitals receiver for UI");
    let wifi_rx = bus.wifi_status.receiver().expect("wifi receiver for UI");
    let gps_rx = bus.gps.receiver().expect("gps receiver for UI");

    spawner
        .spawn(task::render_task(
            config,
            display,
            shared_window,
            window,
            vitals_rx,
            wifi_rx,
            gps_rx,
        ))
        .unwrap();
}
