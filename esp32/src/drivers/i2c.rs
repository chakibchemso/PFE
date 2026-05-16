//! Centralized I²C bus manager — owns the physical bus, creates instrumented
//! device handles, and provides bus scanning and recovery utilities.
//!
//! Every device handle logs transactions via `defmt` so bus activity is
//! fully traceable. Device creation is centralized in [`I2cBus::device`] to
//! keep the peripheral registry explicit and auditable.

use defmt::{Debug2Format, trace, warn};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_hal_async::i2c::{ErrorType, I2c, Operation, SevenBitAddress};
use esp_hal::{Async, i2c::master::I2c as EspI2c};

// ── Type aliases (backward-compatible with bus.rs) ──────────────────────────

pub type SharedI2cBus = Mutex<NoopRawMutex, EspI2c<'static, Async>>;
pub type SharedI2cDevice = I2cDevice<'static, NoopRawMutex, EspI2c<'static, Async>>;
pub type BusError = embassy_embedded_hal::shared_bus::I2cDeviceError<esp_hal::i2c::master::Error>;

// ── I2cBus ──────────────────────────────────────────────────────────────────

/// Central manager for the physical I²C bus.
///
/// Owns the shared [`Mutex`]-wrapped bus. All device handles are created
/// through [`device`](Self::device) — services never touch the bus directly.
pub struct I2cBus {
    bus: Mutex<NoopRawMutex, EspI2c<'static, Async>>,
}

impl I2cBus {
    /// Wrap an already-configured I²C peripheral in a shared bus manager.
    ///
    /// `freq_khz` is kept for documentation / future runtime checks; the
    /// peripheral must already be configured to that speed.
    pub fn new(i2c: EspI2c<'static, Async>, _freq_khz: u32) -> Self {
        Self {
            bus: Mutex::new(i2c),
        }
    }

    /// Create an instrumented device handle for a peripheral.
    ///
    /// The returned [`I2cPeripheral`] implements [`I2c`] and logs every
    /// transaction with the given `name` and `addr`. Clone for retry loops.
    pub fn device(&self, addr: u8, name: &'static str) -> I2cPeripheral {
        // SAFETY: I2cBus is only ever placed in a StaticCell via mk_static!,
        // so self.bus is 'static.
        let bus: &'static Mutex<NoopRawMutex, EspI2c<'static, Async>> =
            unsafe { &*(&self.bus as *const _) };
        I2cPeripheral {
            inner: I2cDevice::new(bus),
            addr,
            name,
        }
    }

    /// Probe all 7-bit addresses (1 … 127) and report which ACK.
    ///
    /// Address 0x00 (general call) is skipped — no real device lives there.
    pub async fn scan(&self) -> [bool; 128] {
        let mut found = [false; 128];
        let mut dev = I2cDevice::new(&self.bus);

        for addr in 1..=0x7Fu8 {
            // Empty write = START + addr[W] + STOP.
            // ACK → device present; NACK → nothing there.
            if dev.write(addr, &[]).await.is_ok() {
                found[addr as usize] = true;
            }
        }
        found
    }

    /// Bus-level recovery: send nine SCL clock cycles to unstick a held-low
    /// SDA line.
    ///
    /// esp-hal already does this automatically on timeout errors. This is a
    /// manual escape hatch for situations where the bus is stuck but no
    /// error has been raised yet (e.g. after a brown-out).
    pub async fn recover(&self) {
        let mut dev = I2cDevice::new(&self.bus);
        // Toggle SCL nine times by addressing dummy devices. Addresses
        // 0x02–0x0A are unlikely to exist; the START/STOP pairs generate
        // SCL pulses that release a slave stretching SDA.
        for addr in 0x02u8..=0x0Au8 {
            let _ = dev.write(addr, &[]).await;
        }
    }
}

// ── I2cPeripheral ───────────────────────────────────────────────────────────

/// Instrumented device handle that logs every I²C transaction.
///
/// Wraps a [`SharedI2cDevice`] and adds `defmt` logging. Uses
/// [`Clone`] (not `Copy` — call `.clone()` explicitly for retry loops).
///
/// Implements [`embedded_hal_async::i2c::I2c`] — a drop-in replacement
/// for [`SharedI2cDevice`] with added observability.
#[derive(Clone)]
pub struct I2cPeripheral {
    inner: I2cDevice<'static, NoopRawMutex, EspI2c<'static, Async>>,
    addr: u8,
    name: &'static str,
}

impl ErrorType for I2cPeripheral {
    type Error = BusError;
}

impl I2c<SevenBitAddress> for I2cPeripheral {
    async fn read(&mut self, address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
        trace!(
            "I2C [{}] @0x{:02X}: read {}B",
            self.name,
            self.addr,
            read.len()
        );
        let result = self.inner.read(address, read).await;
        match &result {
            Ok(_) => trace!("I2C [{}] @0x{:02X}: read OK", self.name, self.addr),
            Err(e) => warn!(
                "I2C [{}] @0x{:02X}: read ERR: {}",
                self.name,
                self.addr,
                Debug2Format(e)
            ),
        }
        result
    }

    async fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        trace!(
            "I2C [{}] @0x{:02X}: write {}B",
            self.name,
            self.addr,
            write.len()
        );
        let result = self.inner.write(address, write).await;
        match &result {
            Ok(_) => trace!("I2C [{}] @0x{:02X}: write OK", self.name, self.addr),
            Err(e) => warn!(
                "I2C [{}] @0x{:02X}: write ERR: {}",
                self.name,
                self.addr,
                Debug2Format(e)
            ),
        }
        result
    }

    async fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Self::Error> {
        trace!(
            "I2C [{}] @0x{:02X}: wr_rd (w:{}B r:{}B)",
            self.name,
            self.addr,
            write.len(),
            read.len()
        );
        let result = self.inner.write_read(address, write, read).await;
        match &result {
            Ok(_) => trace!("I2C [{}] @0x{:02X}: wr_rd OK", self.name, self.addr),
            Err(e) => warn!(
                "I2C [{}] @0x{:02X}: wr_rd ERR: {}",
                self.name,
                self.addr,
                Debug2Format(e)
            ),
        }
        result
    }

    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        trace!(
            "I2C [{}] @0x{:02X}: transaction ({} ops)",
            self.name,
            self.addr,
            operations.len()
        );
        let result = self.inner.transaction(address, operations).await;
        match &result {
            Ok(_) => trace!("I2C [{}] @0x{:02X}: transaction OK", self.name, self.addr),
            Err(e) => warn!(
                "I2C [{}] @0x{:02X}: transaction ERR: {}",
                self.name,
                self.addr,
                Debug2Format(e)
            ),
        }
        result
    }
}

impl I2cPeripheral {
    /// Per-device recovery: send a dummy write to reset the slave's I²C
    /// state machine.
    ///
    /// Sends START + `self.addr`[W] + 0x00 + STOP. Even if the device
    /// NACKs, the START/STOP signaling resets its interface — a NACK on
    /// recovery just means the slave was already idle.
    pub async fn recover(&mut self) {
        let _ = self.inner.write(self.addr, &[0x00]).await;
    }
}
