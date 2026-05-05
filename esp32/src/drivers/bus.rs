use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use esp_hal::{Async, i2c::master::I2c};

pub type SharedI2cBus = Mutex<NoopRawMutex, I2c<'static, Async>>;
pub type SharedI2cDevice = I2cDevice<'static, NoopRawMutex, I2c<'static, Async>>;
