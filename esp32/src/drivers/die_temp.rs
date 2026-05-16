//! ESP32-S3 internal die temperature sensor driver.
//!
//! The ESP32-S3 TSENS peripheral lives in the `SENS` register block (unlike
//! other ESP32 variants where it's in `APB_SARADC`). This driver accesses the
//! registers directly since `esp_hal::tsens` only supports the APB_SARADC
//! variant.

use esp_hal::peripherals::SENS;

/// ESP32-S3 die temperature sensor driver.
pub struct DieTempDriver {
    _sensor: SENS<'static>,
}

impl DieTempDriver {
    pub fn new(sensor: SENS<'static>) -> Self {
        let regs = SENS::regs();

        // Power up the sensor with SW-controlled dump mode.
        regs.sar_tsens_ctrl().modify(|_, w| {
            w.sar_tsens_power_up().set_bit();
            w.sar_tsens_power_up_force().set_bit();
            w.sar_tsens_dump_out().clear_bit()
        });

        Self { _sensor: sensor }
    }

    /// Read die temperature in degrees Celsius.
    ///
    /// Formula from ESP-IDF: `raw * 0.4386 - offset * 27.88 - 20.52`
    /// where offset = -1 for the default measurement range.
    pub fn read_celsius(&self) -> f32 {
        let regs = SENS::regs();

        // Trigger a single measurement
        regs.sar_tsens_ctrl()
            .modify(|_, w| w.sar_tsens_dump_out().set_bit());

        // Wait for measurement to complete
        while !regs.sar_tsens_ctrl().read().sar_tsens_ready().bit_is_set() {
            core::hint::spin_loop();
        }

        let raw = regs.sar_tsens_ctrl().read().sar_tsens_out().bits() as f32;
        let offset: f32 = -1.0;

        raw * 0.4386 - offset * 27.88 - 20.52
    }
}
