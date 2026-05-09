use embassy_executor::Spawner;
use esp_hal::gpio::Input;

use crate::ui::config::RenderConfig;

pub mod driver;
pub mod task;

pub use task::SharedWindowHandle;

/// Spawn the touch input service: I2C init + INT-driven event dispatch.
pub fn register(
    spawner: &Spawner,
    i2c_bus: &'static crate::drivers::bus::SharedI2cBus,
    shared_window: &'static SharedWindowHandle,
    int_pin: Input<'static>,
    config: RenderConfig,
) {
    spawner
        .spawn(task::touch_task(i2c_bus, shared_window, int_pin, config))
        .unwrap();
}
