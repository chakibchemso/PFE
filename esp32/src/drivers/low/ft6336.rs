use embedded_hal_async::i2c::I2c;

const I2C_ADDR: u8 = 0x38;

pub struct Ft6336<I2C> {
    i2c: I2C,
}

impl<I2C: I2c> Ft6336<I2C> {
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    pub async fn init(&mut self) -> Result<(), I2C::Error> {
        // Set Device mode to working mode
        self.write_reg(0x00, 0x00).await?;
        Ok(())
    }

    pub async fn set_period_active(&mut self, period: u8) -> Result<(), I2C::Error> {
        self.write_reg(0x88, period).await
    }

    pub async fn set_period_monitor(&mut self, period: u8) -> Result<(), I2C::Error> {
        self.write_reg(0x89, period).await
    }

    pub async fn set_time_active_monitor(&mut self, time: u8) -> Result<(), I2C::Error> {
        self.write_reg(0x86, time).await
    }

    pub async fn read_touch(&mut self) -> Result<Option<(u16, u16)>, I2C::Error> {
        let mut buf = [0u8; 5];
        // Read TD_STATUS (0x02) to P1_YL (0x06)
        self.i2c.write_read(I2C_ADDR, &[0x02], &mut buf).await?;

        let touches = buf[0] & 0x0F;
        if touches == 0 {
            return Ok(None);
        }

        let xh = buf[1] & 0x0F;
        let xl = buf[2];
        let yh = buf[3] & 0x0F;
        let yl = buf[4];

        let x = ((xh as u16) << 8) | (xl as u16);
        let y = ((yh as u16) << 8) | (yl as u16);

        Ok(Some((x, y)))
    }

    async fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), I2C::Error> {
        self.i2c.write(I2C_ADDR, &[reg, val]).await
    }
}
