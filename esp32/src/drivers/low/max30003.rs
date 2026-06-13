use defmt::info;
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::{Operation, SpiDevice};

const WREG: u8 = 0x00;
const RREG: u8 = 0x01;

// ── Register map ─────────────────────────────────────────────────────────────

#[allow(non_camel_case_types)]
#[derive(Clone, Copy)]
#[repr(u8)]
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

// ── Sampling rate ────────────────────────────────────────────────────────────

#[derive(Debug, Copy, Clone)]
pub enum SamplingRate {
    Sr128 = 128,
    Sr256 = 256,
    Sr512 = 512,
}

// ── FIFO tag (ETAG) constants ────────────────────────────────────────────────

/// ETAG values embedded in the upper bits of each 24-bit FIFO word.
/// Extracted from bits D22:D20 of the sample.
mod etag {
    /// Valid sample data.
    pub const VALID: u8 = 0b000;
    /// Valid sample data (alternate encoding).
    pub const VALID_ALT: u8 = 0b001;
    /// End of FIFO — stop reading.
    pub const EOF: u8 = 0b010;
    /// Fast end of FIFO — stop reading immediately.
    pub const FAST_EOF: u8 = 0b011;
    /// Power-down tag — device entered low-power mode.
    pub const PD: u8 = 0b100;
}

/// Decoded ETAG field from a 24-bit FIFO word.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Tag {
    Valid,
    EndOfFifo,
    FastEof,
    PowerDown,
    Reserved(u8),
}

impl Tag {
    fn from_bits(bits: u8) -> Self {
        match bits {
            etag::VALID | etag::VALID_ALT => Tag::Valid,
            etag::EOF => Tag::EndOfFifo,
            etag::FAST_EOF => Tag::FastEof,
            etag::PD => Tag::PowerDown,
            other => Tag::Reserved(other),
        }
    }
}

/// A single decoded FIFO sample.
///
/// The 18-bit ECG data is sign-extended to `i32`, and the 6 tag bits are
/// decoded into the [`Tag`] enum.
#[derive(Debug, Copy, Clone)]
pub struct FifoWord {
    pub data: i32,
    pub tag: Tag,
}

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error<SpiError> {
    Spi(SpiError),
    InvalidDeviceId,
}

// ── Heart-rate output ────────────────────────────────────────────────────────

#[derive(Debug, Copy, Clone)]
pub struct HeartRateData {
    pub heart_rate: u16,
    pub rr_interval: u16,
}

// ── Driver ───────────────────────────────────────────────────────────────────

pub struct Max30003<SPI> {
    spi: SPI,
}

impl<SPI, SpiError> Max30003<SPI>
where
    SPI: SpiDevice<Error = SpiError>,
{
    /// Create a new MAX30003 driver instance.
    ///
    /// The SPI instance must be an `SpiDevice`, meaning it natively wraps the
    /// CS pin. The MAX30003 CS is typically driven by the TCA9554 I/O expander
    /// (see `drivers::io_expander::IoExpanderService`).
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }

    // ── Register access ──────────────────────────────────────────────────────

    /// Read a 24-bit register from the MAX30003.
    ///
    /// Returns `D[23:0]` packed into a `u32`.
    pub async fn read_register(&mut self, reg: Register) -> Result<u32, Error<SpiError>> {
        let header = ((reg as u8) << 1) | RREG;
        let mut buf = [0u8; 3];

        self.spi
            .transaction(&mut [Operation::Write(&[header]), Operation::Read(&mut buf)])
            .await
            .map_err(Error::Spi)?;

        Ok(((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32))
    }

    /// Write a 24-bit value to a MAX30003 register.
    pub async fn write_register(
        &mut self,
        reg: Register,
        data: u32,
    ) -> Result<(), Error<SpiError>> {
        let header = ((reg as u8) << 1) | WREG;
        let buf = [header, (data >> 16) as u8, (data >> 8) as u8, data as u8];

        self.spi.write(&buf).await.map_err(Error::Spi)
    }

    // ── Status and identification ────────────────────────────────────────────

    /// Read the STATUS register (0x01).
    ///
    /// Bit fields of interest:
    /// - D8  = EINT  (FIFO almost-full flag)
    /// - D9  = EOVF  (FIFO overflow flag)
    /// - D0  = RRTIN (R-to-R timing interrupt)
    ///
    /// Reading clears the interrupt flags.
    pub async fn read_status(&mut self) -> Result<u32, Error<SpiError>> {
        self.read_register(Register::STATUS).await
    }

    /// Read the INFO register and verify the component revision.
    ///
    /// Returns `true` when the upper nibble of the first byte is `0x5`
    /// (expected for the MAX30003).
    pub async fn read_device_id(&mut self) -> Result<bool, Error<SpiError>> {
        let info = self.read_register(Register::INFO).await?;
        let id_byte = (info >> 16) as u8;
        Ok((id_byte & 0xF0) == 0x50)
    }

    // ── Reset ────────────────────────────────────────────────────────────────

    /// Perform a software reset.
    ///
    /// The caller should wait **at least 100 ms** after reset before
    /// accessing registers or starting new configuration.
    pub async fn reset<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<SpiError>> {
        self.write_register(Register::SW_RST, 0x000000).await?;
        delay.delay_ms(100).await;
        Ok(())
    }

    // ── FIFO management ──────────────────────────────────────────────────────

    /// Reset the FIFO by toggling the FIFO_RST register.
    ///
    /// Call this once before starting acquisition to ensure the FIFO pointer
    /// is at a known state. Also useful after recovering from an overflow.
    pub async fn fifo_reset(&mut self) -> Result<(), Error<SpiError>> {
        self.write_register(Register::FIFO_RST, 0x000000).await
    }

    /// Synchronize the ECG channel.
    ///
    /// This starts / restarts data acquisition. The call to `sync()` must be
    /// done **after** all configuration (registers, interrupts, threshold) is
    /// finalised. It is deliberately **not** called from [`begin`](Self::begin)
    /// so that the caller can set up interrupts first.
    pub async fn sync(&mut self) -> Result<(), Error<SpiError>> {
        self.write_register(Register::SYNCH, 0x000000).await
    }

    // ── Initialisation ───────────────────────────────────────────────────────

    /// Run the full power-on initialisation sequence.
    ///
    /// Hardware defaults:
    /// | Register      | Value      | Notes                                    |
    /// |---------------|------------|------------------------------------------|
    /// | `CNFG_GEN`    | `0x080000` | FMSTR = 32 768 Hz, ECG enabled only     |
    /// | `CNFG_CAL`    | `0x000000` | Calibration disabled                     |
    /// | `CNFG_EMUX`   | `0x000000` | Normal inputs (no calibration injection) |
    /// | `CNFG_ECG`    | `0x805000` | 128 sps, 20 V/V gain, DHPF=0.5 Hz, DLPF≈40 Hz |
    /// | `CNFG_RTOR1`  | `0x3FC600` | R‑to‑R enabled, auto‑gain, default timing|
    ///
    /// **`sync()` is NOT called** — the caller must sync explicitly after
    /// finalising interrupt enables, FIFO threshold, and sampling rate.
    pub async fn begin<D: DelayNs>(&mut self, delay: &mut D) -> Result<(), Error<SpiError>> {
        self.reset(delay).await?;
        delay.delay_ms(100).await;

        // Minimal config: ECG enabled, FMSTR = 32768 Hz.
        // All detection/bias features disabled for debugging.
        info!("ECG: write CNFG_GEN = 0x080000");
        self.write_register(Register::CNFG_GEN, 0x080000).await?;
        delay.delay_ms(50).await;

        // Calibration disabled — no test signals injected.
        info!("ECG: write CNFG_CAL = 0x000000");
        self.write_register(Register::CNFG_CAL, 0x000000).await?;
        delay.delay_ms(50).await;

        // Normal input mux: inputs connected, no calibration injection.
        info!("ECG: write CNFG_EMUX = 0x000000");
        self.write_register(Register::CNFG_EMUX, 0x000000).await?;
        delay.delay_ms(50).await;

        // ECG: 128 sps, 20 V/V gain, 0.5 Hz DHPF enabled, ~40 Hz DLPF.
        info!("ECG: write CNFG_ECG = 0x805000");
        self.write_register(Register::CNFG_ECG, 0x805000).await?;
        delay.delay_ms(50).await;

        // R-to-R detection with auto-gain and default timing.
        info!("ECG: write CNFG_RTOR1 = 0x3FC600");
        self.write_register(Register::CNFG_RTOR1, 0x3FC600).await?;
        delay.delay_ms(50).await;

        // Note: sync() is intentionally NOT called here. Call it separately
        // after configuring interrupts, threshold, and rate.

        Ok(())
    }

    // ── Configuration helpers ────────────────────────────────────────────────

    /// Write the full `CNFG_GEN` register (0x10).
    ///
    /// Provides direct control over FMSTR frequency, RBIAS, and other
    /// general‑configuration bits. The reset default is `0x080007`.
    pub async fn configure_gen(&mut self, value: u32) -> Result<(), Error<SpiError>> {
        self.write_register(Register::CNFG_GEN, value).await
    }

    /// Write the full `CNFG_ECG` register (0x15) for complete control over
    /// rate, gain, filter, and lead‑off settings.
    pub async fn configure_ecg(&mut self, value: u32) -> Result<(), Error<SpiError>> {
        self.write_register(Register::CNFG_ECG, value).await
    }

    /// Configure the ECG sampling rate.
    ///
    /// Constructs the full `CNFG_ECG` value internally (no read-back) to
    /// avoid stale data issues. The non‑rate fields (gain, DHPF, DLPF) are
    /// set to the same defaults as [`begin`](Self::begin):
    /// 20 V/V gain, DHPF=0.5 Hz enabled, DLPF≈40 Hz.
    ///
    /// This replaces the previous read‑modify‑write approach that could use
    /// stale register values.
    pub async fn set_sampling_rate(&mut self, rate: SamplingRate) -> Result<(), Error<SpiError>> {
        // Base value: 20 V/V gain, DHPF=0.5 Hz enabled, DLPF=40 Hz.
        const BASE_ECG: u32 = 0x005000;

        let rate_bits = match rate {
            SamplingRate::Sr128 => 0x800000, // bit 23
            SamplingRate::Sr256 => 0x400000, // bit 22
            SamplingRate::Sr512 => 0x000000, // bits 23:21 all zero → 512 sps
        };

        self.write_register(Register::CNFG_ECG, BASE_ECG | rate_bits)
            .await
    }

    /// Set the FIFO interrupt threshold.
    ///
    /// When the number of unread samples in the FIFO reaches `threshold`,
    /// the `EINT` flag is set in the STATUS register.
    ///
    /// `threshold` is clamped to the valid range 1–32.
    pub async fn set_fifo_threshold(&mut self, threshold: u8) -> Result<(), Error<SpiError>> {
        let clamped = threshold.clamp(1, 32);
        // EFIT[4:0] stored in D[23:19], value = threshold - 1.
        let val = ((clamped as u32 - 1) & 0x1F) << 19;
        self.write_register(Register::MNGR_INT, val).await
    }

    /// Configure interrupt enables in the `EN_INT` register (0x02).
    ///
    /// - `eint`:   enable the FIFO almost‑full interrupt (EINTE, D16)
    /// - `eovf`:   enable the FIFO overflow interrupt (EOVFE, D8)
    /// - `rrint`:  enable the R‑to‑R timing interrupt (RRTINE, D0)
    pub async fn enable_interrupts(
        &mut self,
        eint: bool,
        eovf: bool,
        rrint: bool,
    ) -> Result<(), Error<SpiError>> {
        let mut val = 0u32;
        if eint {
            val |= 1 << 16; // D16 = EINTE
        }
        if eovf {
            val |= 1 << 8; // D8 = EOVFE
        }
        if rrint {
            val |= 1 << 0; // D0 = RRTINE
        }
        self.write_register(Register::EN_INT, val).await
    }

    // ── Sample reading ───────────────────────────────────────────────────────

    /// Read a single 24-bit FIFO word from the `ECG_FIFO` register.
    ///
    /// Returns a [`FifoWord`] with the 18-bit ECG data sign-extended to `i32`
    /// and the decoded [`Tag`].
    pub async fn read_ecg_sample(&mut self) -> Result<FifoWord, Error<SpiError>> {
        let raw = self.read_register(Register::ECG_FIFO).await?;

        // D[5:3] = ETAG tag bits (see datasheet Table 32).
        let tag_bits = ((raw >> 3) & 0x07) as u8;

        // D[23:6] = 18-bit ECG data, left-justified two's complement.
        let data_18bit = (raw >> 6) & 0x3FFFF;
        let data = if (data_18bit & 0x20000) != 0 {
            (data_18bit | 0xFFFC_0000) as i32
        } else {
            data_18bit as i32
        };

        Ok(FifoWord {
            data,
            tag: Tag::from_bits(tag_bits),
        })
    }

    /// Read all available ECG samples from the FIFO in burst mode.
    ///
    /// A single SPI transaction reads up to `buffer.len()` bytes (ideally a
    /// multiple of 3, one sample per 3-byte word). The returned data is
    /// scanned for EOF / FAST_EOF markers; samples *after* the first EOF are
    /// discarded.
    ///
    /// Returns the number of **3-byte samples** that were valid (before the
    /// EOF marker). The first `samples_read * 3` bytes of `buffer` contain
    /// the raw sample data, suitable for direct parsing.
    pub async fn read_ecg_fifo(&mut self, buffer: &mut [u8]) -> Result<usize, Error<SpiError>> {
        let header = ((Register::ECG_FIFO_BURST as u8) << 1) | RREG;

        self.spi
            .transaction(&mut [Operation::Write(&[header]), Operation::Read(buffer)])
            .await
            .map_err(Error::Spi)?;

        let max_samples = buffer.len() / 3;
        let mut samples_read = 0;

        for i in 0..max_samples {
            let offset = i * 3;
            // ETAG lives in D[5:3] of the 24-bit word → byte[offset + 2] bits 5:3.
            let tag_bits = (buffer[offset + 2] >> 3) & 0x07;

            match tag_bits {
                etag::VALID | etag::VALID_ALT => {
                    samples_read += 1;
                }
                etag::EOF | etag::FAST_EOF => {
                    break; // EOF marker — stop; word is not a valid sample.
                }
                _ => {
                    // Power-down or reserved tag — count as valid data.
                    samples_read += 1;
                }
            }
        }

        Ok(samples_read)
    }

    /// Raw burst read from the FIFO (no EOF parsing).
    ///
    /// Every 3 bytes in `buffer` forms one 24-bit FIFO word. The caller
    /// should parse ETAG fields themselves.
    ///
    /// Prefer [`read_ecg_fifo`](Self::read_ecg_fifo) which handles EOF
    /// detection automatically.
    pub async fn read_ecg_burst(&mut self, buffer: &mut [u8]) -> Result<(), Error<SpiError>> {
        let header = ((Register::ECG_FIFO_BURST as u8) << 1) | RREG;
        self.spi
            .transaction(&mut [Operation::Write(&[header]), Operation::Read(buffer)])
            .await
            .map_err(Error::Spi)
    }

    // ── Heart-rate / RR interval ─────────────────────────────────────────────

    /// Read the RTOR register and compute heart rate and RR interval.
    ///
    /// Returns `None` when the R‑to‑R interval has not yet been measured
    /// (register reads as zero).
    pub async fn update_heart_rate(&mut self) -> Result<Option<HeartRateData>, Error<SpiError>> {
        let raw = self.read_register(Register::RTOR).await?;

        // RR interval occupies 14 bits starting at D10.
        let rtor = (raw >> 10) & 0x3FFF;

        if rtor == 0 {
            return Ok(None);
        }

        // FMSTR / 256 = 128 Hz → each tick = 7.8125 ms.
        let tick_s = 1.0 / 128.0;
        let rr_seconds = rtor as f32 * tick_s;
        let hr = 60.0 / rr_seconds;
        let rr_ms = rtor as f32 * 7.8125;

        Ok(Some(HeartRateData {
            heart_rate: hr as u16,
            rr_interval: rr_ms as u16,
        }))
    }
}
