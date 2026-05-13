//! LC76G GNSS I2C driver — Quectel I2C protocol.
//!
//! ## I2C addresses (7-bit)
//! - `0x50` — Configuration R/W (write commands, query lengths)
//! - `0x54` — Data read (NMEA output) — *read-only*, won't ACK writes
//! - `0x58` — Data write (PAIR commands, firmware uploads)
//!
//! ## Reading NMEA data (Quectel_Dev_Receive)
//!
//! 1a. Write `[CR_CMD | TX_LEN_REG, 4]` to 0x50  → query available bytes
//! 1b. Read 4 bytes from 0x54                        → u32 LE byte count
//! 2a. Write `[CR_CMD | TX_BUF_REG, chunk_len]` to 0x50  → request chunk
//! 2b. Read `chunk_len` bytes from 0x54                   → NMEA data
//!     Repeat 2a–2b until all available bytes are consumed.
//!
//! ## Commands (u32, little-endian on wire)
//!
//! | Symbol      | Value      |
//! |-------------|------------|
//! | CR_CMD      | 0xAA51     |
//! | CW_CMD      | 0xAA53     |
//! | TX_LEN_REG  | 0x08       |
//! | TX_BUF_REG  | 0x2000     |
//! | RX_LEN_REG  | 0x04       |
//! | RX_BUF_REG  | 0x1000     |

use alloc::vec::Vec;
use core::fmt::Write;

use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

// --- I2C addresses (Quectel §1) ---
const ADDR_CFG: u8 = 0x50; // configuration R/W
const ADDR_RD: u8 = 0x54; // data read (read-only)
const ADDR_WR: u8 = 0x58; // data write

// --- Register offsets ---
const CR_CMD: u32 = 0xAA51;
const CW_CMD: u32 = 0xAA53;
const TX_LEN_REG: u32 = 0x08;
const TX_BUF_REG: u32 = 0x2000;
const RX_LEN_REG: u32 = 0x04;
const RX_BUF_REG: u32 = 0x1000;

const MAX_NMEA_BYTES: usize = 2048;
const CHUNK_SIZE: u32 = 1024;
const MAX_RETRIES: u8 = 10;

/// Build an 8-byte command word pair.
fn cmd_pair(word1: u32, word2: u32) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&word1.to_le_bytes());
    buf[4..].copy_from_slice(&word2.to_le_bytes());
    buf
}

/// Init command: query TX buffer length.
fn init_cmd() -> [u8; 8] {
    cmd_pair((CR_CMD << 16) | TX_LEN_REG, 4)
}

/// Fetch command: request `len` bytes from TX buffer.
fn fetch_cmd(len: u32) -> [u8; 8] {
    cmd_pair((CR_CMD << 16) | TX_BUF_REG, len)
}

pub struct Lc76g<I2C> {
    i2c: I2C,
}

impl<I2C, E> Lc76g<I2C>
where
    I2C: I2c<Error = E>,
    E: core::fmt::Debug,
{
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    /// I2C bus recovery — sends a dummy byte to each of the three module
    /// addresses to reset the slave state machine (Quectel Recovery_I2c).
    pub async fn recover(&mut self) {
        for &addr in &[ADDR_CFG, ADDR_RD, ADDR_WR] {
            let _ = self.i2c.write(addr, &[0x00]).await;
            Timer::after(Duration::from_millis(1)).await;
        }
    }

    /// Probe an I2C address. Tries a write first, then a read (some addresses
    /// are read-only and won't ACK writes).
    pub async fn probe(&mut self, addr: u8) -> Result<bool, E> {
        // Try write probe
        if self.i2c.write(addr, &[]).await.is_ok() {
            return Ok(true);
        }
        // Try read probe (for read-only addresses like 0x54)
        let mut dummy = [0u8; 1];
        if self.i2c.read(addr, &mut dummy).await.is_ok() {
            return Ok(true);
        }
        Ok(false)
    }

    /// Write a command to the config address, retrying on failure with
    /// bus recovery.
    async fn write_cfg(&mut self, cmd: &[u8]) -> Result<(), E> {
        for attempt in 0..MAX_RETRIES {
            match self.i2c.write(ADDR_CFG, cmd).await {
                Ok(_) => return Ok(()),
                Err(_) if attempt < MAX_RETRIES - 1 => {
                    if attempt > 2 {
                        self.recover().await;
                    }
                    Timer::after(Duration::from_millis(10)).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    /// Read bytes from the data address, retrying on failure.
    async fn read_data(&mut self, buf: &mut [u8]) -> Result<(), E> {
        for attempt in 0..MAX_RETRIES {
            match self.i2c.read(ADDR_RD, buf).await {
                Ok(_) => return Ok(()),
                Err(_) if attempt < MAX_RETRIES - 1 => {
                    if attempt > 2 {
                        self.recover().await;
                    }
                    Timer::after(Duration::from_millis(10)).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    /// Poll the LC76G over I2C. Returns the raw NMEA blob if data is available.
    ///
    /// The protocol reads the TX buffer in chunks of `CHUNK_SIZE` (1024 bytes)
    /// until all available data is consumed, matching the Quectel reference.
    pub async fn poll(&mut self) -> Result<Option<Vec<u8>>, E> {
        // Step 1a: query available data length
        self.write_cfg(&init_cmd()).await?;
        Timer::after(Duration::from_millis(10)).await;

        // Step 1b: read 4-byte length
        let mut len_buf = [0u8; 4];
        self.read_data(&mut len_buf).await?;
        let available = u32::from_le_bytes(len_buf) as usize;

        if available == 0 {
            return Ok(None);
        }

        if available > MAX_NMEA_BYTES {
            return Ok(None);
        }

        // Steps 2a–2b: read data in chunks
        let mut data = Vec::with_capacity(available);
        let mut remaining = available;

        while remaining > 0 {
            let chunk = (remaining as u32).min(CHUNK_SIZE);

            self.write_cfg(&fetch_cmd(chunk)).await?;
            Timer::after(Duration::from_millis(10)).await;

            let start = data.len();
            data.resize(start + chunk as usize, 0);
            self.read_data(&mut data[start..]).await?;

            remaining -= chunk as usize;
        }

        Ok(Some(data))
    }

    /// Send a command (e.g. PAIR or PQTM) to the LC76G over I2C.
    ///
    /// Uses the Quectel `Dev_Transmit` protocol:
    /// 1. Query RX buffer free space via 0x50 + 0x54
    /// 2. Write data to 0x58
    pub async fn send_command(&mut self, data: &[u8]) -> Result<(), E> {
        // Step 1a: query RX buffer free space
        let q_cmd = cmd_pair((CR_CMD << 16) | RX_LEN_REG, 4);
        self.write_cfg(&q_cmd).await?;
        Timer::after(Duration::from_millis(10)).await;

        // Step 1b: read free space
        let mut len_buf = [0u8; 4];
        self.read_data(&mut len_buf).await?;
        let free = u32::from_le_bytes(len_buf) as usize;

        // If the buffer doesn't have room, try once more after a short wait
        let free = if free < data.len() {
            Timer::after(Duration::from_millis(100)).await;
            self.write_cfg(&q_cmd).await?;
            Timer::after(Duration::from_millis(10)).await;
            self.read_data(&mut len_buf).await?;
            u32::from_le_bytes(len_buf) as usize
        } else {
            free
        };

        if free < data.len() {
            // Buffer too small — this is unusual; try anyway with the available size
            // but most PAIR commands are under 30 bytes.
        }

        // Step 2a: configure write
        let w_cmd = cmd_pair((CW_CMD << 16) | RX_BUF_REG, data.len() as u32);
        self.write_cfg(&w_cmd).await?;
        Timer::after(Duration::from_millis(10)).await;

        // Step 2b: write data to 0x58
        self.i2c.write(ADDR_WR, data).await
    }
}

// ---------------------------------------------------------------------------
// Coordinate formatting
// ---------------------------------------------------------------------------

fn fmt_lat(lat: f32) -> AsFmt {
    let mut f = AsFmt::new();
    let abs = libm::fabsf(lat);
    let d = libm::truncf(abs) as u8;
    let min = (abs - d as f32) * 60.0;
    let m_int = libm::truncf(min) as u8;
    let m_frac = libm::roundf((min - m_int as f32) * 1_000_000.0) as u32;
    let dir = if lat >= 0.0 { 'N' } else { 'S' };
    let _ = write!(f, "{}°{:02}.{:06}'{}", d, m_int, m_frac, dir);
    f
}

fn fmt_lon(lon: f32) -> AsFmt {
    let mut f = AsFmt::new();
    let abs = libm::fabsf(lon);
    let d = libm::truncf(abs) as u16;
    let min = (abs - d as f32) * 60.0;
    let m_int = libm::truncf(min) as u8;
    let m_frac = libm::roundf((min - m_int as f32) * 1_000_000.0) as u32;
    let dir = if lon >= 0.0 { 'E' } else { 'W' };
    let _ = write!(f, "{}°{:02}.{:06}'{}", d, m_int, m_frac, dir);
    f
}

pub fn format_coords(lat: f32, lon: f32) -> ([u8; 48], usize) {
    let lat_fmt = fmt_lat(lat);
    let lon_fmt = fmt_lon(lon);
    let lat_s = lat_fmt.as_str();
    let lon_s = lon_fmt.as_str();
    let mut buf = [0u8; 48];
    let mut pos = 0;
    for &b in lat_s.as_bytes() {
        buf[pos] = b;
        pos += 1;
    }
    buf[pos] = b' ';
    pos += 1;
    for &b in lon_s.as_bytes() {
        buf[pos] = b;
        pos += 1;
    }
    (buf, pos)
}

struct AsFmt {
    buf: [u8; 16],
    pos: usize,
}

impl AsFmt {
    fn new() -> Self {
        Self {
            buf: [0; 16],
            pos: 0,
        }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("--")
    }
}

impl Write for AsFmt {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.pos + s.len();
        if end > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.pos..end].copy_from_slice(s.as_bytes());
        self.pos = end;
        Ok(())
    }
}
