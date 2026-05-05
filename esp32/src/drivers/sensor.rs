//! Sensor driver initialization: oxymeter and touch sharing a single I2C bus.

use embassy_executor::Spawner;

use crate::drivers::bus::{SharedI2cBus, SharedI2cDevice};
use crate::drivers::oxymeter;
use crate::drivers::touch;

/// Initialize the oxymeter sensor on the shared I2C bus.
pub async fn init_oxymeter(
    spawner: &Spawner,
    i2c_bus: &'static SharedI2cBus,
) -> Result<
    oxymeter::OxymeterHandle,
    embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>,
> {
    let sensor_i2c_device = SharedI2cDevice::new(i2c_bus);
    oxymeter::OxymeterHandle::start(spawner, sensor_i2c_device).await
}

/// Initialize the touch controller on the shared I2C bus.
pub async fn init_touch(
    i2c_bus: &'static SharedI2cBus,
    panel_width: u16,
    panel_height: u16,
) -> Result<touch::TouchDevice, touch::TouchInitError> {
    touch::TouchDevice::new(
        touch::TouchControllerKind::Ft6336,
        i2c_bus,
        panel_width,
        panel_height,
    )
    .await
}
