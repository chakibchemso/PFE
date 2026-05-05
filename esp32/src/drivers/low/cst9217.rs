// CST9217 touch controller async driver

use embedded_hal_async::i2c::I2c;

const CST9217_ADDR: u8 = 0x5A;

// Commands translated from TouchDrvCST92xx.h
const REG_READ: [u8; 2] = [0xD0, 0x00];
const ACK_CMD: [u8; 3] = [0xD0, 0x00, 0xAB]; // 0xAB is CST92XX_ACK
const REG_SLEEP: [u8; 2] = [0xD1, 0x05];
const REG_DEBUG_MODE: [u8; 2] = [0xD1, 0x01];

#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
    pub id: u8,
    pub event: u8,
}

pub struct Cst9217<I2C> {
    i2c: I2C,
}

impl<I2C, E> Cst9217<I2C>
where
    I2C: I2c<Error = E>,
{
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// Reads touch data from the sensor.
    /// ONLY call this immediately after the INT pin goes LOW!
    pub async fn read_touch(&mut self) -> Result<Option<TouchPoint>, E> {
        // MAX_FINGER_NUM (2) * 5 + 5 = 15 bytes
        let mut buffer = [0u8; 15];

        // 1. Write the Read Command and read the 15-byte payload
        self.i2c
            .write_read(CST9217_ADDR, &REG_READ, &mut buffer)
            .await?;

        // 2. The chip requires an ACK immediately after reading, or it locks up
        self.i2c.write(CST9217_ADDR, &ACK_CMD).await?;

        // 3. Validate the payload (translated from SensorLib logic)
        // Buffer[6] must be 0xAB, and Buffer[0] must not be 0xAB
        if buffer[0] == 0xAB || buffer[6] != 0xAB {
            return Ok(None);
        }

        // Check for cover screen gesture (palm mute, etc.)
        if (buffer[4] & 0xF0) >> 7 == 0x01 {
            // Gesture detected, ignoring point data
            return Ok(None);
        }

        // Get number of touch points
        let num_points = buffer[5] & 0x7F;
        if num_points == 0 || num_points > 2 {
            return Ok(None);
        }

        // Parse Point 0
        // Data layout: Byte 0 is Event/ID, Byte 1-3 are 12-bit X/Y coords
        let event = buffer[0] & 0x0F;
        let id = buffer[0] >> 4;

        // 0x06 represents a valid touch event in the CST protocol
        if event == 0x06 {
            let x = ((buffer[1] as u16) << 4) | ((buffer[3] as u16) >> 4);
            let y = ((buffer[2] as u16) << 4) | ((buffer[3] as u16) & 0x0F);

            return Ok(Some(TouchPoint { x, y, id, event }));
        }

        Ok(None)
    }

    /// Forces the chip into deep sleep.
    /// Note: Once in deep sleep, it will only wake up via hardware reset or touch.
    pub async fn sleep(&mut self) -> Result<(), E> {
        // C Driver explicitly sets debug mode before sleeping
        let _ = self.i2c.write(CST9217_ADDR, &REG_DEBUG_MODE).await;
        self.i2c.write(CST9217_ADDR, &REG_SLEEP).await
    }
}
