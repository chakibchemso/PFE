use crate::drivers::bus::{SharedI2cBus, SharedI2cDevice};
use crate::drivers::low::cst9217::Cst9217;
use crate::drivers::low::ft6336::Ft6336;

pub type Ft6336Touch = Ft6336<SharedI2cDevice>;
pub type Cst9217Touch = Cst9217<SharedI2cDevice>;

#[derive(Clone, Copy)]
pub enum TouchControllerKind {
    Ft6336,
    Cst9217,
}

pub enum TouchDevice {
    Ft6336(Ft6336Touch),
    Cst9217(Cst9217Touch),
}

impl TouchDevice {
    pub async fn new(
        controller: TouchControllerKind,
        i2c_bus: &'static SharedI2cBus,
        _panel_width: u16,
        _panel_height: u16,
    ) -> Result<Self, TouchInitError> {
        match controller {
            TouchControllerKind::Ft6336 => {
                let i2c = SharedI2cDevice::new(i2c_bus);
                let mut touch = Ft6336::new(i2c);
                touch.init().await.map_err(TouchInitError::Ft6336)?;
                // Configure touch timing for responsive interaction:
                touch
                    .set_period_active(1)
                    .await
                    .map_err(TouchInitError::Ft6336)?;
                touch
                    .set_period_monitor(14)
                    .await
                    .map_err(TouchInitError::Ft6336)?;
                touch
                    .set_time_active_monitor(0)
                    .await
                    .map_err(TouchInitError::Ft6336)?;
                Ok(Self::Ft6336(touch))
            }
            TouchControllerKind::Cst9217 => {
                let i2c = SharedI2cDevice::new(i2c_bus);
                let touch = Cst9217::new(i2c);
                Ok(Self::Cst9217(touch))
            }
        }
    }

    pub async fn read_touch(&mut self) -> Result<Option<(u16, u16)>, TouchReadError> {
        match self {
            Self::Ft6336(touch) => {
                let event = touch.read_touch().await.map_err(TouchReadError::Ft6336)?;
                Ok(event)
            }
            Self::Cst9217(touch) => {
                let point = touch.read_touch().await.map_err(TouchReadError::Cst9217)?;
                Ok(point.map(|p| (p.x, p.y)))
            }
        }
    }
}

#[derive(Debug)]
pub enum TouchInitError {
    Ft6336(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
}

#[derive(Debug)]
pub enum TouchReadError {
    Ft6336(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
    Cst9217(embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>),
}
