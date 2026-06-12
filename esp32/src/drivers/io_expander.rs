//! Driver abstraction for the TCA9554 I/O expander on the shared I²C bus.
//!
//! Provides a typed wrapper around [`tca9554::Tca9554`] so the rest of the
//! firmware never touches I²C registers directly. Pin assignments for the
//! production board are documented in the [`pins`] module.
//!
//! Also exposes [`IoExpanderService`] — a shared (Mutex‑protected) wrapper
//! that multiple tasks can use to read inputs or drive outputs without a
//! dedicated coordinator task.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;

use crate::drivers::bus::{BusError, I2cPeripheral};
use tca9554::{Address, Tca9554};

// ── Type aliases, constants, pin mappings ────────────────────────────────────

/// Concrete type for our TCA9554 instance.
pub type IoExpander = Tca9554<I2cPeripheral>;

/// I²C address of the TCA9554 on the production board.
///
/// Standard variant (TCA9554, not TCA9554A) with address-select pins
/// A2,A1,A0 all pulled low → `0100_0000` = 0x20.
pub const TCA_ADDR: u8 = 0x20;

/// [`Address`] value matching [`TCA_ADDR`].
pub fn tca_address() -> Address {
    Address::standard().with_selectable_bits((false, false, false))
}

/// Named pin assignments on the TCA9554.
///
/// These mirror the production board schematic — keep in sync if the
/// layout changes.
pub mod pins {
    /// MAX30003 interrupt output (open-drain, active-low). Input on expander.
    pub const MAX30003_INT: u8 = 2;
    /// MAX30102 interrupt output (open-drain, active-low). Input on expander.
    pub const MAX30102_INT: u8 = 0;
    /// MAX30003 chip-select. Output on expander.
    pub const MAX30003_CS: u8 = 1;
    // P3–P7 are spare / board-specific.
}

// ── Individual pin helpers ───────────────────────────────────────────────────

/// Convenience helpers for manipulating individual expander pins.
#[allow(async_fn_in_trait)]
pub trait IoExpanderExt {
    /// Read a single input pin (`true` = logic high).
    ///
    /// For active-low interrupts (MAX30003, MAX30102), the asserted state
    /// reads as `false`.
    async fn read_pin(&mut self, pin: u8) -> Result<bool, BusError>;

    /// Set a single output pin (`true` = drive high).
    async fn set_pin(&mut self, pin: u8, high: bool) -> Result<(), BusError>;

    /// Configure a pin as input or output (`true` = input).
    async fn set_pin_direction(&mut self, pin: u8, input: bool) -> Result<(), BusError>;
}

impl IoExpanderExt for IoExpander {
    async fn read_pin(&mut self, pin: u8) -> Result<bool, BusError> {
        let port = self.read_input().await?;
        Ok((port & (1 << pin)) != 0)
    }

    async fn set_pin(&mut self, pin: u8, high: bool) -> Result<(), BusError> {
        let mut port = self.read_output().await?;
        if high {
            port |= 1 << pin;
        } else {
            port &= !(1 << pin);
        }
        self.write_output(port).await
    }

    async fn set_pin_direction(&mut self, pin: u8, input: bool) -> Result<(), BusError> {
        let mut dir = self.read_direction().await?;
        if input {
            dir |= 1 << pin;
        } else {
            dir &= !(1 << pin);
        }
        self.write_direction(dir).await
    }
}

// ── Shared (Mutex‑protected) service wrapper ─────────────────────────────────

/// Shared I/O expander wrapper.
///
/// Owns the [`IoExpander`] behind a [`Mutex`] so that multiple tasks
/// (sensing, bio‑Z, ECG) can query input pins or drive outputs without
/// a dedicated coordinator task.
pub struct IoExpanderService {
    inner: Mutex<CriticalSectionRawMutex, IoExpander>,
}

impl IoExpanderService {
    /// Create and initialise the expander.
    ///
    /// Takes ownership of an `I2cPeripheral` already configured for the
    /// TCA9554 address (see [`TCA_ADDR`]).
    pub async fn new(i2c: I2cPeripheral) -> Self {
        let mut expander = IoExpander::new(i2c, tca_address());

        // ── Reset to power‑on defaults ─────────────────────────────────
        expander.reset().await.unwrap();

        // ── Pin directions ─────────────────────────────────────────────
        //  0 = output, 1 = input
        //
        //  bit │ pin │ signal          │ direction
        // ─────┼─────┼─────────────────┼──────────
        //  P0  │  0  │ MAX30003_INT    │ 1 (input)
        //  P1  │  1  │ MAX30102_INT    │ 1 (input)
        //  P2  │  2  │ MAX30003_CS     │ 0 (output)
        //  P3  │  3  │ spare           │ 1 (input)
        //  P4  │  4  │ spare           │ 1 (input)
        //  P5  │  5  │ spare           │ 1 (input)
        //  P6  │  6  │ spare           │ 1 (input)
        //  P7  │  7  │ spare           │ 1 (input)
        //
        expander
            .write_direction(0b1111_1011)
            .await
            .expect("TCA9554 direction config failed");

        // ── Outputs to safe defaults ───────────────────────────────────
        // MAX30003_CS = high (inactive / de‑selected)
        expander
            .write_output(1 << pins::MAX30003_CS)
            .await
            .expect("TCA9554 output config failed");

        // Polarity inversion stays at power‑on default (0x00 = normal).

        Self {
            inner: Mutex::new(expander),
        }
    }

    // ── Input API ─────────────────────────────────────────────────────────

    /// Read the entire input port register.
    pub async fn read_input(&self) -> Result<u8, BusError> {
        self.inner.lock().await.read_input().await
    }

    /// Check whether a given input pin is logic‑high.
    pub async fn is_pin_high(&self, pin: u8) -> Result<bool, BusError> {
        let port = self.inner.lock().await.read_input().await?;
        Ok((port & (1 << pin)) != 0)
    }

    /// Check whether the MAX30102 has asserted its interrupt.
    ///
    /// Returns `true` when the pin is **low** (active‑low open‑drain).
    pub async fn is_max30102_int_asserted(&self) -> Result<bool, BusError> {
        let port = self.inner.lock().await.read_input().await?;
        Ok((port & (1 << pins::MAX30102_INT)) == 0)
    }

    /// Check whether the MAX30003 has asserted its interrupt.
    ///
    /// Returns `true` when the pin is **low** (active‑low open‑drain).
    pub async fn is_max30003_int_asserted(&self) -> Result<bool, BusError> {
        let port = self.inner.lock().await.read_input().await?;
        Ok((port & (1 << pins::MAX30003_INT)) == 0)
    }

    // ── Output API ────────────────────────────────────────────────────────

    /// Write the entire output port register.
    pub async fn write_output(&self, state: u8) -> Result<(), BusError> {
        self.inner.lock().await.write_output(state).await
    }

    /// Drive a single output pin high or low.
    pub async fn set_pin(&self, pin: u8, high: bool) -> Result<(), BusError> {
        let mut guard = self.inner.lock().await;
        let mut port = guard.read_output().await?;
        if high {
            port |= 1 << pin;
        } else {
            port &= !(1 << pin);
        }
        guard.write_output(port).await
    }

    /// Assert MAX30003 chip‑select (drive CS low = selected).
    pub async fn select_max30003(&self) -> Result<(), BusError> {
        self.set_pin(pins::MAX30003_CS, false).await // active‑low
    }

    /// De‑assert MAX30003 chip‑select (drive CS high = de‑selected).
    pub async fn deselect_max30003(&self) -> Result<(), BusError> {
        self.set_pin(pins::MAX30003_CS, true).await
    }
}
