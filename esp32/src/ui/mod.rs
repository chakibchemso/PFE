mod config;
mod pipeline;
mod platform;
mod touch_task;

pub mod widgets {
    pub mod gps;
    pub mod watchface;
}

pub use config::{RenderConfig, RoundMaskLUT};
pub use pipeline::{SharedWindow, ui_task};
pub use platform::init_platform;
pub use touch_task::{SharedWindowHandle, touch_task};
pub use widgets::gps::GpsState;
pub use widgets::watchface::{ClockState, tick as clock_tick};

slint::include_modules!();

pub const PRODUCTION_UI_SIZE: u16 = 466;

/// Shared SPI bus type for async DMA access
pub type DisplaySpiBus = embassy_sync::mutex::Mutex<
    embassy_sync::blocking_mutex::raw::NoopRawMutex,
    esp_hal::spi::master::SpiDmaBus<'static, esp_hal::Async>,
>;

/// Embassy async SpiDevice wrapper for shared bus access
pub type DisplaySpiDevice = embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice<
    'static,
    embassy_sync::blocking_mutex::raw::NoopRawMutex,
    esp_hal::spi::master::SpiDmaBus<'static, esp_hal::Async>,
    esp_hal::gpio::Output<'static>,
>;

/// Display interface type (lcd-async async SPI interface)
pub type DisplayInterface =
    lcd_async::interface::SpiInterface<DisplaySpiDevice, esp_hal::gpio::Output<'static>>;

/// Full display type
pub type SmartWatchDisplay =
    lcd_async::Display<DisplayInterface, lcd_async::models::ST7796, esp_hal::gpio::Output<'static>>;
