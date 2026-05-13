//! CST9217 / CST9220 Capacitive Touch Controller Driver
//!
//! Async `embedded-hal` driver for the Hynitron CST92xx series capacitive touch
//! controllers (CST9217, CST9220). Communicates over I²C at address `0x5A`.
//!
//! # Features
//! - Async I²C via `embedded-hal-async`
//! - Up to 2 concurrent touch points
//! - Coordinate transformations (swap XY, mirror X/Y, scaling)
//! - Sleep / wake-up support
//! - Run-mode switching (Normal, Factory, Debug, etc.)
//! - Bootloader entry and memory read for firmware updates
//! - Cover-screen / home-button gesture detection
//!
//! # Example
//! ```rust,ignore
//! use cst9217::Cst9217;
//! use embedded_hal_async::delay::DelayNs;
//! use embedded_hal_async::i2c::I2c;
//!
//! async fn example(i2c: impl I2c, delay: impl DelayNs, rst: impl OutputPin) {
//!     let mut touch = Cst9217::new(i2c, delay, Some(rst), 0x5A);
//!     touch.init().await.unwrap();
//!     touch.set_swap_xy(true);
//!     touch.set_mirror_xy(false, true);
//!
//!     loop {
//!         let data = touch.read_touch().await.unwrap();
//!         for (i, pt) in data.points.iter().flatten().enumerate() {
//!             defmt::info!("Finger {}: x={}, y={}", i, pt.x, pt.y);
//!         }
//!         if data.home_button {
//!             defmt::info!("Home button pressed!");
//!         }
//!     }
//! }
//! ```
//!
//! # Register Map Summary
//!
//! The CST92xx uses a page + offset addressing scheme over I²C. The first byte
//! sent is the page (command group), the second is the offset within that page.
//!
//! | Page | Offset | Purpose |
//! |------|--------|---------|
//! | 0xD0 | 0x00   | Touch data read-out (REG_READ) |
//! | 0xD1 | 0x01   | Enter command / debug info mode |
//! | 0xD1 | 0x05   | Sleep mode register |
//! | 0xD1 | 0x09   | Normal mode |
//! | 0xD1 | 0x0A   | Raw data mode |
//! | 0xD1 | 0x0D   | Diff data mode |
//! | 0xD1 | 0x14   | Factory mode |
//! | 0xD1 | 0xF8   | Resolution X/Y (read) |
//! | 0xD1 | 0xFC   | Checkcode (read) |
//! | 0xD2 | 0x04   | Chip type + Project ID (read) |
//! | 0xD2 | 0x08   | Firmware version + checksum (read) |
//! | 0xA0 | 0x01   | Bootloader handshake |
//! | 0xA0 | 0x04   | Bootloader status / poll |
//! | 0xA0 | 0x10   | Bootloader memory read setup |
//! | 0xA0 | 0x18   | Bootloader memory read data |
//!
//! # Touch Data Format
//!
//! Reading 15 bytes from `0xD000` yields:
//!
//! ```text
//! [0..3]   Finger 0: [id:4|event:4], x[11:4], y[11:4], x[3:0]:y[3:0]
//! [4]      Gesture / cover-screen flags
//! [5]      Number of touch points (mask 0x7F)
//! [6]      ACK status (expected 0xAB)
//! [7..10]  Finger 1: same 4-byte layout as finger 0
//! [11..14] Reserved / padding
//! ```
//!
//! # Coordinate Pipeline
//!
//! Transformations are applied in this order (matching the C++ reference):
//! 1. **Swap XY** – exchange X and Y axes.
//! 2. **Scale** – map raw panel resolution to target display resolution.
//! 3. **Mirror X/Y** – flip around the centre of `x_max` / `y_max`.
//! 4. **Clamp** – constrain to `[0, x_max]` and `[0, y_max]`.

use core::fmt;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::{delay::DelayNs, i2c::I2c};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Default I²C slave address.
pub const DEFAULT_SLAVE_ADDRESS: u8 = 0x5A;

/// Bootloader / probe address (same as normal address for this family).
pub const BOOT_ADDRESS: u8 = 0x5A;

/// Maximum number of simultaneous touch points supported by the hardware.
pub const MAX_FINGER_NUM: usize = 2;

/// Expected ACK byte in the touch data stream.
const CST92XX_ACK: u8 = 0xAB;

/// Size of a single touch data read (2 fingers × 5 + 5 overhead).
const TOUCH_READ_LEN: usize = MAX_FINGER_NUM * 5 + 5;

// ── 16-bit register addresses (page << 8 | offset) ──────────────────────────

const REG_READ: u16 = 0xD000;
const REG_DEBUG_MODE: u16 = 0xD101;
const REG_SLEEP_MODE: u16 = 0xD105;

const REG_NORMAL_MODE: u16 = 0xD109;
const REG_RAW_MODE: u16 = 0xD10A;
const REG_DIFF_MODE: u16 = 0xD10D;

const REG_LOW_POWER_MODE: u16 = 0xD10F;
const REG_FACTORY_MODE: u16 = 0xD114;

// ── Chip identifiers ─────────────────────────────────────────────────────────

const CST9220_CHIP_ID: u16 = 0x9220;
const CST9217_CHIP_ID: u16 = 0x9217;

// ── Bootloader constants ─────────────────────────────────────────────────────

const BL_CMD_HANDSHAKE: u8 = 0xA0;
const BL_SUB_HANDSHAKE: u8 = 0x01;
const BL_SUB_STATUS: u8 = 0x04;
const BL_SUB_MEM_SETUP: u8 = 0x10;
const BL_SUB_MEM_ADDR: u8 = 0x0C;

const BL_SUB_MEM_READ: u8 = 0x18;
const BL_TRIG_READ: u8 = 0xE4;
const BL_HS_REQ: u8 = 0xAA;
const BL_HS_RESP_0: u8 = 0x55;
const BL_HS_RESP_1: u8 = 0xB0;

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// Errors returned by the driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<I2cErr> {
    /// Underlying I²C bus error.
    I2c(I2cErr),
    /// The chip reported an ID that does not match CST9217 / CST9220.
    InvalidChipId,
    /// Firmware is missing (version reads as `0xA5A5A5A5`).
    NoFirmware,
    /// Firmware info / checkcode verification failed.
    FirmwareInfoError,
    /// Failed to set the requested run mode.
    ModeSetFailed,
    /// Failed to enter bootloader.
    BootloaderFailed,
    /// An unexpected response was received from the controller.
    InvalidResponse,
}

impl<I2cErr: fmt::Debug> fmt::Display for Error<I2cErr> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::I2c(e) => write!(f, "I2C error: {:?}", e),
            Error::InvalidChipId => write!(f, "invalid chip ID"),
            Error::NoFirmware => write!(f, "no firmware detected"),
            Error::FirmwareInfoError => write!(f, "firmware info verification failed"),
            Error::ModeSetFailed => write!(f, "mode set failed"),
            Error::BootloaderFailed => write!(f, "bootloader entry failed"),
            Error::InvalidResponse => write!(f, "invalid controller response"),
        }
    }
}

impl<I2cErr: defmt::Format> defmt::Format for Error<I2cErr> {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Error::I2c(e) => defmt::write!(fmt, "I2C error: {}", e),
            Error::InvalidChipId => defmt::write!(fmt, "invalid chip ID"),
            Error::NoFirmware => defmt::write!(fmt, "no firmware detected"),
            Error::FirmwareInfoError => defmt::write!(fmt, "firmware info verification failed"),
            Error::ModeSetFailed => defmt::write!(fmt, "mode set failed"),
            Error::BootloaderFailed => defmt::write!(fmt, "bootloader entry failed"),
            Error::InvalidResponse => defmt::write!(fmt, "invalid controller response"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data types
// ─────────────────────────────────────────────────────────────────────────────

/// A single detected touch point.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TouchPoint {
    /// X coordinate (0 .. resolution_x after scaling/clamping).
    pub x: u16,
    /// Y coordinate (0 .. resolution_y after scaling/clamping).
    pub y: u16,
    /// Pressure value (always 0 for CST92xx – not supported by hardware).
    pub pressure: u16,
    /// Touch point tracking ID.
    pub id: u8,
    /// Raw event code from the controller.
    ///
    /// Known values:
    /// - `0x06` – finger down / contact
    /// - `0x00` – finger up / release
    pub event: u8,
}

/// Container for a full touch sample.
#[derive(Debug, Clone, Copy)]
pub struct TouchData {
    /// Detected touch points. `None` slots are unused.
    pub points: [Option<TouchPoint>; MAX_FINGER_NUM],
    /// Number of valid touch points (0 ..= 2).
    pub num_points: u8,
    /// Set when a cover-screen home-button gesture is detected.
    pub home_button: bool,
}

impl Default for TouchData {
    fn default() -> Self {
        Self {
            points: [None; MAX_FINGER_NUM],
            num_points: 0,
            home_button: false,
        }
    }
}

/// Identified controller model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// CST9217
    Cst9217,
    /// CST9220
    Cst9220,
    /// Unknown / unrecognised chip type.
    Unknown,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Model::Cst9217 => write!(f, "CST9217"),
            Model::Cst9220 => write!(f, "CST9220"),
            Model::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Operating modes that can be selected with [`Cst9217::set_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RunMode {
    /// Normal touch reporting mode.
    Normal = 0x00,
    /// Low-power mode.
    LowPower = 0x01,
    /// Deep sleep.
    DeepSleep = 0x02,
    /// Wake-up sequence.
    Wakeup = 0x03,
    /// Debug: output diff data.
    DebugDiff = 0x04,
    /// Debug: output raw sensor data.
    DebugRawData = 0x05,
    /// Factory test mode.
    Factory = 0x06,
    /// Debug info / command mode.
    DebugInfo = 0x07,
    /// Firmware update mode.
    UpdateFw = 0x08,
    /// Factory test: high drive.
    FactoryHighDrv = 0x10,
    /// Factory test: low drive.
    FactoryLowDrv = 0x11,
    /// Factory test: short detection.
    FactoryShort = 0x12,
    /// Low-power scan.
    LpScan = 0x13,
}

/// Coordinate-transformation configuration.
#[derive(Debug, Clone, Copy)]
pub struct TouchConfig {
    swap_xy: bool,
    mirror_x: bool,
    mirror_y: bool,
    scaling_enabled: bool,
    x_max: u16,
    y_max: u16,
    resolution_x: u16,
    resolution_y: u16,
    scale_x: f32,
    scale_y: f32,
}

impl Default for TouchConfig {
    fn default() -> Self {
        Self {
            swap_xy: false,
            mirror_x: false,
            mirror_y: false,
            scaling_enabled: false,
            x_max: 0,
            y_max: 0,
            resolution_x: 0,
            resolution_y: 0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Driver
// ─────────────────────────────────────────────────────────────────────────────

/// CST9217 / CST9220 async touch driver.
///
/// `I2C` must implement [`I2c`], `DELAY` must implement [`DelayNs`], and
/// `RST` (if provided) must implement [`OutputPin`] with an infallible error
/// type. If your platform returns a fallible GPIO error, wrap the pin in a
/// thin adapter that panics or discards the error.
pub struct Cst9217<I2C, DELAY, RST> {
    i2c: I2C,
    delay: DELAY,
    reset: Option<RST>,
    addr: u8,
    chip_id: u16,
    chip_type: u16,
    resolution_x: u16,
    resolution_y: u16,
    fw_version: u32,
    checksum: u32,
    config: TouchConfig,
}

impl<I2C, DELAY, RST, I2cErr> Cst9217<I2C, DELAY, RST>
where
    I2C: I2c<Error = I2cErr>,
    DELAY: DelayNs,
    RST: OutputPin<Error = core::convert::Infallible>,
{
    // ── Constructors ─────────────────────────────────────────────────────────

    /// Create a new driver instance.
    ///
    /// * `i2c` – async I²C bus.
    /// * `delay` – async delay provider.
    /// * `reset` – optional reset GPIO (active-low assumed; toggled in [`Self::reset`]).
    /// * `addr` – I²C slave address (usually `0x5A`).
    pub fn new(i2c: I2C, delay: DELAY, reset: Option<RST>, addr: u8) -> Self {
        Self {
            i2c,
            delay,
            reset,
            addr,
            chip_id: 0,
            chip_type: 0,
            resolution_x: 0,
            resolution_y: 0,
            fw_version: 0,
            checksum: 0,
            config: TouchConfig::default(),
        }
    }

    /// Release the peripheral resources.
    pub fn free(self) -> (I2C, DELAY, Option<RST>) {
        (self.i2c, self.delay, self.reset)
    }

    // ── Initialization ───────────────────────────────────────────────────────

    /// Initialise the controller.
    ///
    /// Performs a hardware reset (if a reset pin was supplied), waits for the
    /// controller to boot, reads the chip attributes (checkcode, resolution,
    /// chip type, firmware version) and validates them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidChipId`] if the detected chip is neither
    /// CST9217 nor CST9220, [`Error::NoFirmware`] if the firmware slot is
    /// empty, or [`Error::FirmwareInfoError`] if the checkcode is wrong.
    pub async fn init(&mut self) -> Result<(), Error<I2cErr>> {
        self.reset_controller().await;
        self.delay.delay_ms(30).await;

        // Enter command mode
        self.write_reg(0xD1, 0x01).await?;
        self.delay.delay_ms(10).await;

        // Read checkcode @ 0xD1FC
        let mut buf = [0u8; 4];
        self.write_then_read(&[0xD1, 0xFC], &mut buf).await?;
        let checkcode = u32::from_le_bytes(buf);

        // Read resolution @ 0xD1F8
        self.write_then_read(&[0xD1, 0xF8], &mut buf).await?;
        self.resolution_x = u16::from_le_bytes([buf[0], buf[1]]);
        self.resolution_y = u16::from_le_bytes([buf[2], buf[3]]);

        // Read chip type + Project ID @ 0xD204
        self.write_then_read(&[0xD2, 0x04], &mut buf).await?;
        self.chip_type = u16::from_le_bytes([buf[2], buf[3]]);
        let _project_id = u16::from_le_bytes([buf[0], buf[1]]);

        // Read fw version + checksum @ 0xD208
        let mut buf8 = [0u8; 8];
        self.write_then_read(&[0xD2, 0x08], &mut buf8).await?;
        self.fw_version = u32::from_le_bytes([buf8[0], buf8[1], buf8[2], buf8[3]]);
        self.checksum = u32::from_le_bytes([buf8[4], buf8[5], buf8[6], buf8[7]]);

        // Validation
        if self.fw_version == 0xA5A5A5A5 {
            return Err(Error::NoFirmware);
        }
        if (checkcode & 0xFFFF_0000) != 0xCACA_0000 {
            return Err(Error::FirmwareInfoError);
        }
        if self.chip_type != CST9217_CHIP_ID && self.chip_type != CST9220_CHIP_ID {
            return Err(Error::InvalidChipId);
        }

        self.chip_id = self.chip_type;
        Ok(())
    }

    // ── Touch reading ────────────────────────────────────────────────────────

    /// Poll the controller for the current touch state.
    ///
    /// This performs the full I²C transaction (write read-address, read 15 B,
    /// write ACK) and applies the configured coordinate transformations.
    ///
    /// # Errors
    ///
    /// Returns an I²C error if the bus transaction fails.
    pub async fn read_touch(&mut self) -> Result<TouchData, Error<I2cErr>> {
        let mut data = TouchData::default();

        // Write read address 0xD000 then read 15 bytes
        let mut buf = [0u8; TOUCH_READ_LEN];
        self.write_then_read(&[(REG_READ >> 8) as u8, REG_READ as u8], &mut buf)
            .await?;

        // Write ACK: [0xD0, 0x00, 0xAB]
        self.i2c
            .write(self.addr, &[(REG_READ >> 8) as u8, REG_READ as u8, CST92XX_ACK])
            .await
            .map_err(Error::I2c)?;

        // Validate frame
        if buf[0] == CST92XX_ACK || buf[6] != CST92XX_ACK {
            return Ok(data);
        }

        // Cover-screen / home-button gesture
        if buf[4] & 0xF0 != 0 {
            if (buf[4] >> 7) == 0x01 {
                data.home_button = true;
                return Ok(data);
            }
        }

        let num_points = buf[5] & 0x7F;
        if num_points == 0 || num_points as usize > MAX_FINGER_NUM {
            return Ok(data);
        }

        for i in 0..num_points as usize {
            let offset = i * 5 + if i == 0 { 0 } else { 2 };
            let pdat = &buf[offset..offset + 4];

            let id = pdat[0] >> 4;
            let event = pdat[0] & 0x0F;
            let x = ((pdat[1] as u16) << 4) | ((pdat[3] as u16) >> 4);
            let y = ((pdat[2] as u16) << 4) | ((pdat[3] as u16) & 0x0F);

            if event == 0x06 && id < MAX_FINGER_NUM as u8 {
                data.points[i] = Some(TouchPoint {
                    x,
                    y,
                    pressure: 0,
                    id,
                    event,
                });
                data.num_points += 1;
            }
        }

        // If the first point reports "up", discard everything
        if let Some(first) = data.points[0] {
            if first.event == 0x00 {
                return Ok(TouchData::default());
            }
        }

        // Apply coordinate transformations
        self.apply_transformations(&mut data);

        Ok(data)
    }

    // ── Power management ─────────────────────────────────────────────────────

    /// Put the controller into sleep mode.
    ///
    /// Enters debug-info mode and writes the sleep-mode register. If no reset
    /// pin is connected the chip cannot be woken up without a power cycle.
    pub async fn sleep(&mut self) -> Result<(), Error<I2cErr>> {
        self.set_mode(RunMode::DebugInfo).await?;
        self.i2c
            .write(
                self.addr,
                &[(REG_SLEEP_MODE >> 8) as u8, REG_SLEEP_MODE as u8],
            )
            .await
            .map_err(Error::I2c)?;
        Ok(())
    }

    /// Wake the controller by toggling the reset pin.
    ///
    /// If no reset pin was provided this method does nothing; you must power
    /// cycle the board instead.
    pub async fn wakeup(&mut self) {
        self.reset_controller().await;
    }

    // ── Run-mode switching ───────────────────────────────────────────────────

    /// Change the controller operating mode.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ModeSetFailed`] if the controller does not acknowledge
    /// the mode change.
    pub async fn set_mode(&mut self, mode: RunMode) -> Result<(), Error<I2cErr>> {
        let mut read_buf = [0u8; 4];
        let mode_cmd: u8;

        // Unlock / sync: write 0xD11E twice, then read 0x0002 until byte[1]==0x1E
        for _ in 0..3 {
            self.i2c
                .write(self.addr, &[0xD1, 0x1E])
                .await
                .map_err(Error::I2c)?;
            self.i2c
                .write(self.addr, &[0xD1, 0x1E])
                .await
                .map_err(Error::I2c)?;

            self.write_then_read(&[0x00, 0x02], &mut read_buf).await?;
            if read_buf[1] == 0x1E {
                break;
            }
            self.delay.delay_ms(200).await;
        }

        let write_buf: [u8; 2] = match mode {
            RunMode::Normal => {
                mode_cmd = REG_NORMAL_MODE as u8;
                [(REG_NORMAL_MODE >> 8) as u8, REG_NORMAL_MODE as u8]
            }
            RunMode::DebugDiff => {
                mode_cmd = REG_DIFF_MODE as u8;
                [(REG_DIFF_MODE >> 8) as u8, REG_DIFF_MODE as u8]
            }
            RunMode::DebugRawData => {
                mode_cmd = REG_RAW_MODE as u8;
                [(REG_RAW_MODE >> 8) as u8, REG_RAW_MODE as u8]
            }
            RunMode::DebugInfo => {
                mode_cmd = REG_DEBUG_MODE as u8;
                [(REG_DEBUG_MODE >> 8) as u8, REG_DEBUG_MODE as u8]
            }
            RunMode::Factory => {
                mode_cmd = REG_FACTORY_MODE as u8;
                [(REG_FACTORY_MODE >> 8) as u8, REG_FACTORY_MODE as u8]
            }
            RunMode::FactoryLowDrv => {
                mode_cmd = 0x11;
                [0xD1, 0x11]
            }
            RunMode::FactoryHighDrv => {
                mode_cmd = 0x10;
                [0xD1, 0x10]
            }
            RunMode::FactoryShort => {
                mode_cmd = 0x12;
                [0xD1, 0x12]
            }
            RunMode::LowPower => {
                mode_cmd = REG_LOW_POWER_MODE as u8;
                [(REG_LOW_POWER_MODE >> 8) as u8, REG_LOW_POWER_MODE as u8]
            }
            RunMode::DeepSleep => {
                mode_cmd = REG_SLEEP_MODE as u8;
                [(REG_SLEEP_MODE >> 8) as u8, REG_SLEEP_MODE as u8]
            }
            RunMode::LpScan => {
                mode_cmd = 0x13;
                [0xD1, 0x13]
            }
            _ => {
                // Wakeup, UpdateFw, etc. – not handled by simple register write
                return Ok(());
            }
        };

        // Factory mode needs extra polling
        if mode == RunMode::Factory {
            for _ in 0..10 {
                self.i2c
                    .write(self.addr, &write_buf)
                    .await
                    .map_err(Error::I2c)?;
                self.delay.delay_ms(10).await;
                self.write_then_read(&[0x00, 0x09], &mut read_buf).await?;
                if read_buf[0] == 0x14 {
                    break;
                }
                self.delay.delay_ms(1).await;
            }
            self.i2c
                .write(self.addr, &[0xD1, 0x19])
                .await
                .map_err(Error::I2c)?;
        } else {
            self.i2c
                .write(self.addr, &write_buf)
                .await
                .map_err(Error::I2c)?;
        }

        // Verify
        self.delay.delay_ms(10).await;
        self.write_then_read(&[0x00, 0x02], &mut read_buf).await?;
        if mode_cmd != 0 && read_buf[1] != mode_cmd {
            return Err(Error::ModeSetFailed);
        }

        self.delay.delay_ms(10).await;
        Ok(())
    }

    // ── Bootloader / firmware update helpers ─────────────────────────────────

    /// Enter the built-in bootloader.
    ///
    /// This is required before calling [`Self::read_word_from_mem`]. The
    /// function toggles reset and performs the handshake sequence
    /// (`0xAA` → `0x55 0xB0`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::BootloaderFailed`] if the handshake does not succeed.
    pub async fn enter_bootloader(&mut self) -> Result<(), Error<I2cErr>> {
        let saved_addr = self.addr;
        if self.addr != BOOT_ADDRESS {
            self.addr = BOOT_ADDRESS;
        }

        let mut read_buf = [0u8; 2];

        for delay_ms in (10..=20).step_by(2) {
            self.reset_controller().await;
            self.delay.delay_ms(delay_ms).await;

            for _ in 0..5 {
                // Handshake request
                let _ = self
                    .i2c
                    .write(self.addr, &[BL_CMD_HANDSHAKE, BL_SUB_HANDSHAKE, BL_HS_REQ])
                    .await;
                self.delay.delay_ms(2).await;

                // Read response
                let res = self
                    .write_then_read(
                        &[BL_CMD_HANDSHAKE, BL_SUB_HANDSHAKE + 1],
                        &mut read_buf,
                    )
                    .await;
                if res.is_ok() && read_buf[0] == BL_HS_RESP_0 && read_buf[1] == BL_HS_RESP_1 {
                    // Exit handshake
                    let _ = self
                        .i2c
                        .write(self.addr, &[BL_CMD_HANDSHAKE, BL_SUB_HANDSHAKE, 0x00])
                        .await;
                    self.addr = saved_addr;
                    return Ok(());
                }
                self.delay.delay_ms(2).await;
            }
        }

        self.addr = saved_addr;
        Err(Error::BootloaderFailed)
    }

    /// Read a 32-bit word from controller memory via the bootloader.
    ///
    /// Only works after successfully calling [`Self::enter_bootloader`] and
    /// only when the device address is `0x5A`.
    ///
    /// * `mem_type` – memory type byte (passed as the third byte of the setup).
    /// * `mem_addr` – 16-bit memory address.
    pub async fn read_word_from_mem(
        &mut self,
        mem_type: u8,
        mem_addr: u16,
    ) -> Result<u32, Error<I2cErr>> {
        let saved_addr = self.addr;
        if self.addr != BOOT_ADDRESS {
            self.addr = BOOT_ADDRESS;
        }

        let mut read_buf = [0u8; 4];

        // Setup: [0xA0, 0x10, type]
        self.i2c
            .write(self.addr, &[BL_CMD_HANDSHAKE, BL_SUB_MEM_SETUP, mem_type])
            .await
            .map_err(Error::I2c)?;

        // Address: [0xA0, 0x0C, addr_lo, addr_hi]
        self.i2c
            .write(
                self.addr,
                &[
                    BL_CMD_HANDSHAKE,
                    BL_SUB_MEM_ADDR,
                    mem_addr as u8,
                    (mem_addr >> 8) as u8,
                ],
            )
            .await
            .map_err(Error::I2c)?;

        // Trigger: [0xA0, 0x04, 0xE4]
        self.i2c
            .write(
                self.addr,
                &[BL_CMD_HANDSHAKE, BL_SUB_STATUS, BL_TRIG_READ],
            )
            .await
            .map_err(Error::I2c)?;

        // Poll until status == 0x00
        for _ in 0..100 {
            self.write_then_read(
                &[BL_CMD_HANDSHAKE, BL_SUB_STATUS],
                &mut read_buf[..1],
            )
            .await?;
            if read_buf[0] == 0x00 {
                break;
            }
        }

        // Read data: [0xA0, 0x18] → 4 bytes
        self.write_then_read(
            &[BL_CMD_HANDSHAKE, BL_SUB_MEM_READ],
            &mut read_buf,
        )
        .await?;

        self.addr = saved_addr;
        Ok(u32::from_le_bytes(read_buf))
    }

    // ── Coordinate transformations ───────────────────────────────────────────

    /// Enable or disable X/Y swapping.
    pub fn set_swap_xy(&mut self, swap: bool) {
        self.config.swap_xy = swap;
    }

    /// Current swap-XY state.
    pub fn swap_xy(&self) -> bool {
        self.config.swap_xy
    }

    /// Enable or disable axis mirroring.
    pub fn set_mirror_xy(&mut self, mirror_x: bool, mirror_y: bool) {
        self.config.mirror_x = mirror_x;
        self.config.mirror_y = mirror_y;
    }

    /// Current mirror state.
    pub fn mirror_xy(&self) -> (bool, bool) {
        (self.config.mirror_x, self.config.mirror_y)
    }

    /// Set the maximum coordinates used for mirroring and clamping.
    ///
    /// These should match your display dimensions.
    pub fn set_max_coordinates(&mut self, x: u16, y: u16) {
        self.config.x_max = x;
        self.config.y_max = y;
    }

    /// Get the current maximum coordinates.
    pub fn max_coordinates(&self) -> (u16, u16) {
        (self.config.x_max, self.config.y_max)
    }

    /// Set the raw panel resolution.
    ///
    /// Call this before [`Self::set_target_resolution`] if you want scaling.
    pub fn set_resolution(&mut self, x: u16, y: u16) {
        self.config.resolution_x = x;
        self.config.resolution_y = y;
    }

    /// Raw panel resolution.
    pub fn resolution(&self) -> (u16, u16) {
        (self.config.resolution_x, self.config.resolution_y)
    }

    /// Enable scaling by defining the target display resolution.
    ///
    /// Scaling factors are computed from the previously set raw resolution.
    /// If raw resolution has not been set, scaling is disabled.
    pub fn set_target_resolution(&mut self, width: u16, height: u16) {
        if self.config.resolution_x == 0 || self.config.resolution_y == 0 {
            self.config.scale_x = 1.0;
            self.config.scale_y = 1.0;
            self.config.scaling_enabled = false;
        } else {
            self.config.scale_x = width as f32 / self.config.resolution_x as f32;
            self.config.scale_y = height as f32 / self.config.resolution_y as f32;
            self.config.scaling_enabled = true;
        }
        self.config.x_max = width;
        self.config.y_max = height;
    }

    /// Whether scaling is currently enabled.
    pub fn scaling_enabled(&self) -> bool {
        self.config.scaling_enabled
    }

    // ── Introspection ────────────────────────────────────────────────────────

    /// Detected chip model.
    pub fn model(&self) -> Model {
        match self.chip_id {
            CST9217_CHIP_ID => Model::Cst9217,
            CST9220_CHIP_ID => Model::Cst9220,
            _ => Model::Unknown,
        }
    }

    /// Raw chip type register value.
    pub fn chip_type(&self) -> u16 {
        self.chip_type
    }

    /// Firmware version read during init.
    pub fn fw_version(&self) -> u32 {
        self.fw_version
    }

    /// Firmware checksum read during init.
    pub fn checksum(&self) -> u32 {
        self.checksum
    }

    /// Maximum number of touch points supported (always 2 for this family).
    pub const fn max_touch_points(&self) -> u8 {
        MAX_FINGER_NUM as u8
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Hardware reset via the RST pin.
    async fn reset_controller(&mut self) {
        if let Some(ref mut rst) = self.reset {
            rst.set_low().ok();
            self.delay.delay_ms(10).await;
            rst.set_high().ok();
            self.delay.delay_ms(10).await;
        }
    }

    /// Write a single byte to an 8-bit register address.
    async fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), Error<I2cErr>> {
        self.i2c
            .write(self.addr, &[reg, val])
            .await
            .map_err(Error::I2c)
    }

    /// I²C write-then-read transaction.
    async fn write_then_read(
        &mut self,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error<I2cErr>> {
        self.i2c
            .write_read(self.addr, write, read)
            .await
            .map_err(Error::I2c)
    }

    /// Apply the configured coordinate pipeline to all valid points.
    fn apply_transformations(&self, data: &mut TouchData) {
        if data.num_points == 0 {
            return;
        }

        for slot in &mut data.points {
            let Some(pt) = slot else { continue };

            // 1. Swap XY
            if self.config.swap_xy {
                core::mem::swap(&mut pt.x, &mut pt.y);
            }

            // 2. Scale
            if self.config.scaling_enabled {
                pt.x = (pt.x as f32 * self.config.scale_x + 0.5) as u16;
                pt.y = (pt.y as f32 * self.config.scale_y + 0.5) as u16;
            }

            // 3. Mirror X
            if self.config.mirror_x && self.config.x_max > 0 {
                pt.x = self.config.x_max.saturating_sub(pt.x);
            }

            // 4. Mirror Y
            if self.config.mirror_y && self.config.y_max > 0 {
                pt.y = self.config.y_max.saturating_sub(pt.y);
            }

            // 5. Clamp
            if self.config.x_max != 0 && pt.x > self.config.x_max {
                pt.x = self.config.x_max;
            }
            if self.config.y_max != 0 && pt.y > self.config.y_max {
                pt.y = self.config.y_max;
            }
        }
    }
}
