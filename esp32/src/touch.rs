use core::cell::RefCell;

use embassy_embedded_hal::shared_bus::blocking::i2c::I2cDevice;
use embassy_sync::blocking_mutex::{Mutex, raw::NoopRawMutex};
use esp_hal::{Blocking, i2c::master::I2c};
use ft6x36::{Dimension, Ft6x36, RawTouchEvent};

pub type SharedI2cBus = Mutex<NoopRawMutex, RefCell<I2c<'static, Blocking>>>;
pub type SharedI2cDevice = I2cDevice<'static, NoopRawMutex, I2c<'static, Blocking>>;
pub type Ft6336Touch = Ft6x36<SharedI2cDevice>;

#[derive(Clone, Copy)]
pub enum TouchControllerKind {
    Ft6336,
    Cst916,
}

pub enum TouchDevice {
    Ft6336(Ft6336Touch),
}

impl TouchDevice {
    pub fn new(
        controller: TouchControllerKind,
        i2c: SharedI2cDevice,
        panel_width: u16,
        panel_height: u16,
    ) -> Result<Self, TouchInitError> {
        match controller {
            TouchControllerKind::Ft6336 => {
                let mut touch = Ft6x36::new(i2c, Dimension(panel_width, panel_height));
                touch.init().map_err(TouchInitError::Ft6336)?;
                // Configure touch timing for responsive interaction:
                // - PeriodActive=1: ~6ms report rate when touching (fastest)
                // - PeriodMonitor=14: ~14ms when idle (catches initial press quickly)
                // - TimeActiveMonitor=0: don't switch to slow monitor mode
                touch.set_period_active(1).map_err(TouchInitError::Ft6336)?;
                touch
                    .set_period_monitor(14)
                    .map_err(TouchInitError::Ft6336)?;
                touch
                    .set_time_active_monitor(0)
                    .map_err(TouchInitError::Ft6336)?;
                Ok(Self::Ft6336(touch))
            }
            TouchControllerKind::Cst916 => Err(TouchInitError::UnsupportedController),
        }
    }

    pub fn read_touch(&mut self) -> Result<Option<(u16, u16)>, TouchReadError> {
        let event = match self {
            Self::Ft6336(touch) => touch.get_touch_event().map_err(TouchReadError::Ft6336)?,
        };

        Ok(primary_touch(event))
    }
}

#[derive(Debug)]
pub enum TouchInitError {
    Ft6336(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
    UnsupportedController,
}

#[derive(Debug)]
pub enum TouchReadError {
    Ft6336(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
}

fn primary_touch(event: RawTouchEvent) -> Option<(u16, u16)> {
    event.p1.map(|point| (point.x, point.y))
}
