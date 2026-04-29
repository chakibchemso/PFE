//! System-level initialization: clocks, heap, RTOS timer.

pub mod board;

use esp_hal::peripherals::Peripherals;

/// System initialization: clocks, heap allocator.
/// Returns the peripherals for board-level init.
pub fn init_system() -> Peripherals {
    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default()
        .with_cpu_clock(esp_hal::clock::CpuClock::max())
        .with_psram(esp_hal::psram::PsramConfig {
            flash_frequency: esp_hal::psram::FlashFreq::FlashFreq80m,
            ram_frequency: esp_hal::psram::SpiRamFreq::Freq80m,
            core_clock: Some(
                esp_hal::psram::SpiTimingConfigCoreClock::SpiTimingConfigCoreClock160m,
            ),
            ..Default::default()
        });
    let p = esp_hal::init(config);

    // Initialize heap: internal SRAM + PSRAM
    {
        // Internal SRAM heap in regular DRAM.
        esp_alloc::heap_allocator!(size: 64 * 1024);
        // Additional SRAM reclaimed from unused ROM sections.
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
        // PSRAM heap (16MB on ESP32-S3N32R16V)
        esp_alloc::psram_allocator!(p.PSRAM, esp_hal::psram);
    }

    p
}
