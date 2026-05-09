use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::{Operation, SpiDevice};

const WREG: u8 = 0x00;
const RREG: u8 = 0x01;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
pub enum Register {
    STATUS = 0x01,
    EN_INT = 0x02,
    EN_INT2 = 0x03,
    MNGR_INT = 0x04,
    MNGR_DYN = 0x05,
    SW_RST = 0x08,
    SYNCH = 0x09,
    FIFO_RST = 0x0A,
    INFO = 0x0F,
    CNFG_GEN = 0x10,
    CNFG_CAL = 0x12,
    CNFG_EMUX = 0x14,
    CNFG_ECG = 0x15,
    CNFG_RTOR1 = 0x1D,
    CNFG_RTOR2 = 0x1E,
    ECG_FIFO_BURST = 0x20,
    ECG_FIFO = 0x21,
    RTOR = 0x25,
}

#[derive(Debug, Copy, Clone)]
pub enum SamplingRate {
    Sr128 = 128,
    Sr256 = 256,
    Sr512 = 512,
}

#[derive(Debug)]
pub enum Error<SpiError> {
    Spi(SpiError),
    InvalidDeviceId,
}

#[derive(Debug, Copy, Clone)]
pub struct HeartRateData {
    pub heart_rate: u16,
    pub rr_interval: u16,
}

pub struct Max30003<SPI> {
    spi: SPI,
}

impl<SPI, SpiError> Max30003<SPI>
where
    SPI: SpiDevice<Error = SpiError>,
{
    /// Create a new MAX30003 driver instance.
    /// Note: The SPI instance must be an `SpiDevice`, meaning it natively wraps the CS pin.
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }

    /// Read a 24-bit register from the MAX30003
    pub async fn read_register(&mut self, reg: Register) -> Result<u32, Error<SpiError>> {
        let header = ((reg as u8) << 1) | RREG;
        let mut buf = [0u8; 3];

        self.spi
            .transaction(&mut [Operation::Write(&[header]), Operation::Read(&mut buf)])
            .await
            .map_err(Error::Spi)?;

        Ok(((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32))
    }

    /// Write a 24-bit value to a MAX30003 register
    pub async fn write_register(
        &mut self,
        reg: Register,
        data: u32,
    ) -> Result<(), Error<SpiError>> {
        let header = ((reg as u8) << 1) | WREG;
        let buf = [header, (data >> 16) as u8, (data >> 8) as u8, data as u8];

        self.spi.write(&buf).await.map_err(Error::Spi)
    }

    /// Read the device ID to confirm communication and component revision
    pub async fn read_device_id(&mut self) -> Result<bool, Error<SpiError>> {
        let info = self.read_register(Register::INFO).await?;
        // Device revision in upper nibble of the first byte should be 0x5
        // info >> 16 gives us the first byte (buf[0] from the C++ driver)
        let id_byte = (info >> 16) as u8;
        Ok((id_byte & 0xF0) == 0x50)
    }

    /// Perform a software reset
    pub async fn reset<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<SpiError>> {
        self.write_register(Register::SW_RST, 0x000000).await?;
        delay.delay_ms(100).await;
        Ok(())
    }

    /// Synchronize the ECG channel
    pub async fn sync(&mut self) -> Result<(), Error<SpiError>> {
        self.write_register(Register::SYNCH, 0x000000).await
    }

    /// Complete initialization sequence (power-on config)
    pub async fn begin<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<SpiError>> {
        self.reset(delay).await?;
        delay.delay_ms(100).await;

        self.write_register(Register::CNFG_GEN, 0x081007).await?;
        delay.delay_ms(50).await;

        self.write_register(Register::CNFG_CAL, 0x720000).await?;
        delay.delay_ms(50).await;

        self.write_register(Register::CNFG_EMUX, 0x0B0000).await?;
        delay.delay_ms(50).await;

        self.write_register(Register::CNFG_ECG, 0x805000).await?;
        delay.delay_ms(50).await;

        self.write_register(Register::CNFG_RTOR1, 0x3FC600).await?;
        delay.delay_ms(50).await;

        self.sync().await?;
        delay.delay_ms(50).await;

        Ok(())
    }

    /// Change the sampling rate dynamically
    pub async fn set_sampling_rate(&mut self, rate: SamplingRate) -> Result<(), Error<SpiError>> {
        let mut current_cfg = self.read_register(Register::CNFG_ECG).await?;

        // Clear the sample rate bits (MSB)
        current_cfg &= 0x3FFFFF;

        match rate {
            SamplingRate::Sr128 => current_cfg |= 0x800000,
            SamplingRate::Sr256 => current_cfg |= 0x400000,
            SamplingRate::Sr512 => { /* leave as 0 */ }
        }

        self.write_register(Register::CNFG_ECG, current_cfg).await
    }

    /// Read a single 24-bit signed ECG sample
    pub async fn read_ecg_sample(&mut self) -> Result<i32, Error<SpiError>> {
        let raw = self.read_register(Register::ECG_FIFO).await?;

        // Sign-extend 24-bit to 32-bit
        let mut sample = raw as i32;
        if (sample & 0x80_0000) != 0 {
            sample |= 0xFF00_0000_u32 as i32;
        }

        Ok(sample)
    }

    /// Reads an arbitrary length buffer from the ECG FIFO burst.
    /// The buffer length should ideally be a multiple of 3 bytes.
    pub async fn read_ecg_burst(&mut self, buffer: &mut [u8]) -> Result<(), Error<SpiError>> {
        let header = ((Register::ECG_FIFO_BURST as u8) << 1) | RREG;
        self.spi
            .transaction(&mut [Operation::Write(&[header]), Operation::Read(buffer)])
            .await
            .map_err(Error::Spi)
    }

    /// Updates and calculates the heart rate and RR interval from the RTOR register.
    /// Returns `None` if the register has no valid R-to-R interval yet.
    pub async fn update_heart_rate(&mut self) -> Result<Option<HeartRateData>, Error<SpiError>> {
        let raw = self.read_register(Register::RTOR).await?;

        // Extract the RR interval.
        // In C++: ((reg[0] << 8) | reg[1]) >> 2
        // Since `raw` is (reg[0] << 16) | (reg[1] << 8) | reg[2],
        // shifting right by 10 achieves the same bits:
        let rtor = (raw >> 10) & 0x3FFF;

        if rtor == 0 {
            return Ok(None);
        }

        let hr = 60.0 / (rtor as f32 * 0.0078125);
        let rr = rtor as f32 * 7.8125;

        Ok(Some(HeartRateData {
            heart_rate: hr as u16,
            rr_interval: rr as u16,
        }))
    }
}
