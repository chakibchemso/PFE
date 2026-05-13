use embassy_executor::Spawner;

use crate::app::bus::SystemBus;
use crate::drivers::bus::SharedI2cBus;

pub mod task;

/// Spawn the GPS service: periodically polls the LC76G via the shared I2C
/// bus and publishes fix data to `bus.gps`.
pub fn register(spawner: &Spawner, i2c_bus: &'static SharedI2cBus, bus: &'static SystemBus) {
    let gps_tx = bus.gps.sender();

    spawner.spawn(task::gps_task(i2c_bus, gps_tx)).unwrap();
}
