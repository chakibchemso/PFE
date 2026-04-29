//! Sensor driver initialization: oxymeter and touch sharing a single I2C bus.

use embassy_executor::Spawner;

use crate::drivers::oxymeter;
use crate::drivers::touch::{self, SharedI2cBus};

/// Initialize the oxymeter sensor on the shared I2C bus.
pub async fn init_oxymeter(
    spawner: &Spawner,
    i2c_bus: &'static SharedI2cBus,
) -> Result<
    oxymeter::OxymeterHandle,
    embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>,
> {
    let sensor_i2c_device = touch::SharedI2cDevice::new(i2c_bus);
    oxymeter::OxymeterHandle::start(spawner, sensor_i2c_device).await
}

/// Initialize the touch controller on the shared I2C bus.
pub fn init_touch(
    i2c_bus: &'static SharedI2cBus,
    panel_width: u16,
    panel_height: u16,
) -> Result<touch::TouchDevice, touch::TouchInitError> {
    let touch_i2c_device = touch::SharedI2cDevice::new(i2c_bus);

    touch::TouchDevice::new(
        touch::TouchControllerKind::Ft6336,
        touch_i2c_device,
        panel_width,
        panel_height,
    )
}
