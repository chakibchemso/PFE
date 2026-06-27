//! NTC thermistor driver (MF52-103 3435).
//!
//! # Hardware
//! - NTC: MF52-103 3435 (10 kΩ at 25 °C, B = 3435)
//! - Series resistor to GND: 10 kΩ
//! - NTC pulled to 3.3 V
//! - Middle tap → GPIO3 (ADC1_CH2)
//!
//! # Temperature calculation
//! 1. Read 12‑bit ADC → bias-corrected raw counts
//! 2. Voltage divider → NTC resistance
//! 3. B‑parameter equation → temperature in °C

use core::sync::atomic::AtomicU8;

use esp_hal::Blocking;
use esp_hal::analog::adc::{Adc, AdcCalBasic, AdcPin, Attenuation};
use esp_hal::peripherals::{ADC1, GPIO3};
use libm::logf;

// ── Shared state ──────────────────────────────────────────────────────────

/// Latest skin temperature in °C, written by the NTC task and read by the
/// oxymeter runner when sending vitals. Defaults to 36 °C as a safe fallback.
pub static SKIN_TEMP: AtomicU8 = AtomicU8::new(36);

// ── NTC constants ─────────────────────────────────────────────────────────

/// Supply voltage (V).
const VCC: f32 = 3.3;

/// 12‑bit ADC full scale.
const ADC_RANGE: f32 = 4095.0;

/// Series resistor to GND (Ω).
const R_REF: f32 = 15_000.0;

/// NTC resistance at 25 °C (Ω).
const R25: f32 = 10_000.0;

/// B‑parameter (B25/85).
const B: f32 = 3435.0;

/// 25 °C in Kelvin.
const T0: f32 = 298.15;

/// Number of ADC samples to average (matching MicroPython reference).
const ADC_SAMPLES: u32 = 32;

// ── Driver ────────────────────────────────────────────────────────────────

/// NTC thermistor driver that owns the ADC1 peripheral and the GPIO3 pin.
pub struct NtcDriver {
    adc: Adc<'static, ADC1<'static>, Blocking>,
    pin: AdcPin<GPIO3<'static>, ADC1<'static>, AdcCalBasic<ADC1<'static>>>,
}

impl NtcDriver {
    pub fn new(
        adc: Adc<'static, ADC1<'static>, Blocking>,
        pin: AdcPin<GPIO3<'static>, ADC1<'static>, AdcCalBasic<ADC1<'static>>>,
    ) -> Self {
        Self { adc, pin }
    }

    /// Read the current temperature in °C.
    ///
    /// Averages 32 ADC samples, then converts to temperature via the
    /// B‑parameter equation.
    pub fn read_celsius(&mut self) -> f32 {
        let mut sum: u32 = 0;
        for _ in 0..ADC_SAMPLES {
            sum += self.adc.read_blocking(&mut self.pin) as u32;
        }
        let avg_raw = (sum / ADC_SAMPLES) as u16;
        raw_to_celsius(avg_raw)
    }
}

/// Create an [`NtcDriver`] from raw ADC1 and GPIO3 peripherals.
pub fn create_ntc_driver(adc1: ADC1<'static>, gpio3: GPIO3<'static>) -> NtcDriver {
    use esp_hal::analog::adc::AdcConfig;

    let mut config = AdcConfig::new();
    let pin = config.enable_pin_with_cal::<_, AdcCalBasic<_>>(gpio3, Attenuation::_11dB);
    let adc = Adc::new(adc1, config);
    NtcDriver { adc, pin }
}

/// Convert a raw 12‑bit ADC reading to °C using the B‑parameter equation.
fn raw_to_celsius(raw: u16) -> f32 {
    let v_adc = (raw as f32) * VCC / ADC_RANGE;

    // Voltage divider (NTC to 3.3V, reference resistor to GND):
    //   V_ADC = VCC * R_REF / (R_NTC + R_REF)
    let r_ntc = R_REF * (VCC / v_adc - 1.0);
    let r_ntc = r_ntc.clamp(100.0, 1_000_000.0);

    // B‑parameter equation: 1/T = 1/T0 + 1/B * ln(R/R25)
    let t_k = 1.0 / (1.0 / T0 + (1.0 / B) * logf(r_ntc / R25));
    t_k - 273.15
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At 25 °C the NTC = 10 kΩ, voltage divider gives 1.65 V → ADC ≈ 2048.
    #[test]
    fn test_25c() {
        let temp = raw_to_celsius(2048);
        assert!((temp - 25.0).abs() < 2.0, "got {temp}°C at 2048");
    }

    /// At body temperature (~37 °C), R_NTC ≈ 6.5 kΩ, ADC ≈ 2450.
    #[test]
    fn test_body_temp() {
        let temp = raw_to_celsius(2450);
        assert!(temp > 30.0 && temp < 45.0, "got {temp}°C at 2450");
    }

    /// Very cold, ADC ≈ 300.
    #[test]
    fn test_cold() {
        let temp = raw_to_celsius(300);
        assert!(temp > -10.0 && temp < 15.0, "got {temp}°C at 300");
    }
}
