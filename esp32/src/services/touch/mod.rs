use embassy_executor::Spawner;
use esp_hal::gpio::{Input, Output};

use crate::drivers::bus::I2cPeripheral;
use crate::ui::config::RenderConfig;

pub mod driver;
pub mod task;

pub use task::SharedWindowHandle;

/// Spawn the touch input service: I2C init + INT-driven event dispatch.
pub fn register(
    spawner: &Spawner,
    i2c: I2cPeripheral,
    shared_window: &'static SharedWindowHandle,
    int_pin: Input<'static>,
    touch_rst: Output<'static>,
    config: RenderConfig,
) {
    spawner.spawn(task::touch_task(i2c, shared_window, int_pin, touch_rst, config).unwrap());
}
