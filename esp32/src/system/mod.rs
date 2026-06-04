//! System-level initialization: clocks, heap, RTOS timer, networking.

pub mod board;
pub mod net;

use esp_hal::{Config, clock::CpuClock, peripherals::Peripherals};

/// System initialization: clocks, heap allocator.
/// Returns the peripherals for board-level init.
pub fn init_system() -> Peripherals {
    rtt_target::rtt_init_defmt!();

    let config = Config::default().with_cpu_clock(CpuClock::_240MHz);
    let p = esp_hal::init(config);

    p
}
